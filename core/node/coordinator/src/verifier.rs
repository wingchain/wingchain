// Copyright 2019, 2020 Wingchain
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashSet;
use std::sync::Arc;

use node_chain::ChainCommitBlockParams;
use primitives::errors::{Catchable, CommonResult, Display};
use primitives::{BlockNumber, BuildBlockParams, FullTransaction, Hash, Header, Transaction};

use crate::errors;
use crate::errors::ErrorKind;
use crate::protocol::{BlockData, BodyData};
use crate::stream::StreamSupport;
use crate::support::CoordinatorSupport;

pub struct Verifier<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	support: Arc<StreamSupport<S>>,
}

impl<S> Verifier<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	pub fn new(support: Arc<StreamSupport<S>>) -> CommonResult<Self> {
		let verifier = Self { support };
		Ok(verifier)
	}
}

#[derive(Debug, Display)]
pub enum VerifyError {
	/// Block duplicated
	#[display(fmt = "Duplicated")]
	Duplicated,
	/// Block is not the best
	#[display(fmt = "Not best")]
	NotBest,
	/// Bad block
	#[display(fmt = "Bad")]
	Bad,
	/// Invalid execution gap
	#[display(fmt = "Invalid exection gap")]
	InvalidExecutionGap,
	/// Should wait executing
	#[display(fmt = "Should wait")]
	ShouldWait,
	/// Invalid header
	#[display(fmt = "Invalid header: {}", _0)]
	InvalidHeader(String),
	/// Transaction duplicated
	#[display(fmt = "Duplicated tx: {}", _0)]
	DuplicatedTx(String),
	/// Transaction invalid
	#[display(fmt = "Invalid tx: {}", _0)]
	InvalidTx(String),
}

impl<S> Verifier<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	/// block_data may be taken
	pub fn verify_block(
		&self,
		block_data: &mut Option<BlockData>,
	) -> CommonResult<ChainCommitBlockParams> {
		{
			let block_data_ref = block_data.as_ref().expect("qed");
			self.verify_data(block_data_ref)?;
			let block_hash = &block_data_ref.block_hash;
			let header = block_data_ref.header.as_ref().expect("qed");

			self.verify_not_repeat(block_hash)?;
			let (_confirmed_number, _confirmed_hash, confirmed_header) =
				self.verify_best(header)?;

			self.verify_execution(header, &confirmed_header)?;
		}

		// the following verification need take ownership of block data
		let block_data = block_data.take().expect("qed");
		let header = block_data.header.expect("qed");
		let body = block_data.body.expect("qed");

		let (meta_txs, payload_txs) = self.verify_body(body)?;

		let commit_block_params = self.verify_header(&header, meta_txs, payload_txs)?;

		Ok(commit_block_params)
	}

	fn verify_data(&self, block_data: &BlockData) -> CommonResult<()> {
		let _header = match &block_data.header {
			Some(v) => v,
			None => return Err(ErrorKind::VerifyError(VerifyError::Bad).into()),
		};

		let _body = match &block_data.body {
			Some(v) => v,
			None => return Err(ErrorKind::VerifyError(VerifyError::Bad).into()),
		};

		Ok(())
	}

	fn verify_not_repeat(&self, block_hash: &Hash) -> CommonResult<()> {
		if self.support.ori_support().get_header(block_hash)?.is_some() {
			return Err(ErrorKind::VerifyError(VerifyError::Duplicated).into());
		}
		Ok(())
	}

	/// Return confirmed block (number, block hash, header)
	fn verify_best(&self, header: &Header) -> CommonResult<(BlockNumber, Hash, Header)> {
		let confirmed = {
			let confirmed_number = self
				.support
				.ori_support()
				.get_confirmed_number()?
				.ok_or(errors::ErrorKind::Data(format!("Missing confirmed number")))?;
			let block_hash = self
				.support
				.ori_support()
				.get_block_hash(&confirmed_number)?
				.ok_or(errors::ErrorKind::Data(format!(
					"Missing block hash: number: {}",
					confirmed_number
				)))?;
			let header = self.support.ori_support().get_header(&block_hash)?.ok_or(
				errors::ErrorKind::Data(format!("Missing header: block_hash: {:?}", block_hash)),
			)?;
			(confirmed_number, block_hash, header)
		};

		if !(header.number == confirmed.0 + 1 && &header.parent_hash == &confirmed.1) {
			return Err(ErrorKind::VerifyError(VerifyError::NotBest).into());
		}

		Ok(confirmed)
	}

	fn verify_execution(&self, header: &Header, confirmed_header: &Header) -> CommonResult<()> {
		let system_meta = &self.support.ori_support().get_current_state().system_meta;

		if header.payload_execution_gap < 1 {
			return Err(ErrorKind::VerifyError(VerifyError::InvalidExecutionGap).into());
		}

		if header.payload_execution_gap > system_meta.max_execution_gap {
			return Err(ErrorKind::VerifyError(VerifyError::InvalidExecutionGap).into());
		}

		// execution number of the verifying block
		let execution_number = header.number - header.payload_execution_gap as u64;

		// execution number of the confirmed block
		let confirmed_execution_number =
			confirmed_header.number - confirmed_header.payload_execution_gap as u64;
		if execution_number < confirmed_execution_number {
			return Err(ErrorKind::VerifyError(VerifyError::InvalidExecutionGap).into());
		}

		// execution number of current state
		let current_execution_number =
			self.support
				.ori_support()
				.get_execution_number()?
				.ok_or(errors::ErrorKind::Data(format!(
					"Execution number not found"
				)))?;
		if execution_number > current_execution_number {
			return Err(ErrorKind::VerifyError(VerifyError::ShouldWait).into());
		}

		Ok(())
	}

	/// Return verified txs (meta_txs, payload_txs)
	fn verify_body(
		&self,
		body: BodyData,
	) -> CommonResult<(Vec<Arc<FullTransaction>>, Vec<Arc<FullTransaction>>)> {
		let get_verified_txs = |txs: Vec<Transaction>| -> CommonResult<Vec<Arc<FullTransaction>>> {
			let mut set = HashSet::new();
			let mut result = Vec::with_capacity(txs.len());
			for tx in txs {
				let tx_hash = self.support.ori_support().hash_transaction(&tx)?;
				self.verify_transaction(&tx_hash, &tx, &mut set)?;
				let tx = Arc::new(FullTransaction { tx_hash, tx });
				result.push(tx);
			}
			Ok(result)
		};

		let meta_txs = get_verified_txs(body.meta_txs)?;

		let payload_txs = get_verified_txs(body.payload_txs)?;

		Ok((meta_txs, payload_txs))
	}

	/// return commit block params
	fn verify_header(
		&self,
		header: &Header,
		meta_txs: Vec<Arc<FullTransaction>>,
		payload_txs: Vec<Arc<FullTransaction>>,
	) -> CommonResult<ChainCommitBlockParams> {
		let execution_number = header.number - header.payload_execution_gap as u64;

		let build_block_params = BuildBlockParams {
			number: header.number,
			timestamp: header.timestamp,
			meta_txs,
			payload_txs,
			execution_number,
		};

		let commit_block_params = self.support.ori_support().build_block(build_block_params)?;

		if &commit_block_params.header != header {
			let msg = format!(
				"Invalid header: {:?}, expected: {:?}",
				header, commit_block_params.header
			);
			return Err(ErrorKind::VerifyError(VerifyError::InvalidHeader(msg)).into());
		}

		Ok(commit_block_params)
	}

	fn verify_transaction(
		&self,
		tx_hash: &Hash,
		tx: &Transaction,
		set: &mut HashSet<Hash>,
	) -> CommonResult<()> {
		if !set.insert(tx_hash.clone()) {
			return Err(ErrorKind::VerifyError(VerifyError::DuplicatedTx(format!(
				"Duplicated tx: {}",
				tx_hash
			)))
			.into());
		}

		self.support
			.ori_support()
			.validate_transaction(tx_hash, &tx, true)
			.or_else_catch::<node_chain::errors::ErrorKind, _>(|e| match e {
				node_chain::errors::ErrorKind::ValidateTxError(e) => Some(Err(
					ErrorKind::VerifyError(VerifyError::InvalidTx(e.to_string())).into(),
				)),
				_ => None,
			})?;

		Ok(())
	}
}
