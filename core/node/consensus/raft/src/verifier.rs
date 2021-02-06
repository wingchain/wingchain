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

use crate::errors::ErrorKind;
use crate::protocol::Proposal;
use node_chain::ChainCommitBlockParams;
use node_consensus_base::support::ConsensusSupport;
use primitives::errors::{Catchable, CommonResult, Display};
use primitives::types::ExecutionGap;
use primitives::{BlockNumber, BuildBlockParams, FullTransaction, Hash, Header, Transaction};
use std::collections::HashSet;
use std::sync::Arc;

pub struct Verifier<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
}

#[derive(Debug, Display)]
pub enum VerifyError {
	#[display(fmt = "Duplicated")]
	Duplicated,
	/// Block is not the best
	#[display(fmt = "Not best")]
	NotBest,
	/// Invalid execution gap
	#[display(fmt = "Invalid execution gap")]
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
	InvalidTx(node_chain::errors::ValidateTxError),
}

impl<S> Verifier<S>
where
	S: ConsensusSupport,
{
	pub fn new(support: Arc<S>) -> CommonResult<Self> {
		let verifier = Self { support };
		Ok(verifier)
	}
}

impl<S> Verifier<S>
where
	S: ConsensusSupport,
{
	/// proposal may be taken
	pub fn verify_proposal(
		&self,
		proposal: &mut Option<Proposal>,
	) -> CommonResult<(Proposal, ChainCommitBlockParams)> {
		{
			let proposal_ref = proposal.as_ref().expect("qed");
			let block_hash = &proposal_ref.block_hash;
			let number = proposal_ref.number;
			let execution_number = proposal_ref.execution_number;

			self.verify_not_repeat(block_hash)?;
			let (_confirmed_number, _confirmed_hash, confirmed_header) =
				self.verify_best(number)?;

			self.verify_execution(number, execution_number, &confirmed_header)?;
		}

		// the following verification need take ownership of proposal
		let proposal = proposal.take().expect("qed");
		let proposal_clone = proposal.clone();

		let (meta_txs, payload_txs) = self.verify_body(proposal.meta_txs, proposal.payload_txs)?;

		let commit_block_params = self.verify_header(
			&proposal.block_hash,
			proposal.number,
			proposal.timestamp,
			proposal.execution_number,
			meta_txs,
			payload_txs,
		)?;

		Ok((proposal_clone, commit_block_params))
	}

	fn verify_not_repeat(&self, block_hash: &Hash) -> CommonResult<()> {
		if self.support.get_header(block_hash)?.is_some() {
			return Err(ErrorKind::VerifyError(VerifyError::Duplicated).into());
		}
		Ok(())
	}

	/// Return confirmed block (number, block hash, header)
	fn verify_best(&self, number: BlockNumber) -> CommonResult<(BlockNumber, Hash, Header)> {
		let confirmed = {
			let current_state = &self.support.get_current_state();
			let confirmed_number = current_state.confirmed_number;
			let block_hash = current_state.confirmed_block_hash.clone();
			let header = self.support.get_header(&block_hash)?.ok_or_else(|| {
				node_consensus_base::errors::ErrorKind::Data(format!(
					"Missing header: block_hash: {:?}",
					block_hash
				))
			})?;
			(confirmed_number, block_hash, header)
		};

		if number != confirmed.0 + 1 {
			return Err(ErrorKind::VerifyError(VerifyError::NotBest).into());
		}

		Ok(confirmed)
	}

	fn verify_execution(
		&self,
		number: BlockNumber,
		execution_number: BlockNumber,
		confirmed_header: &Header,
	) -> CommonResult<()> {
		let current_state = self.support.get_current_state();
		let system_meta = &current_state.system_meta;
		let payload_execution_gap = (number - execution_number) as ExecutionGap;

		if payload_execution_gap < 1 {
			return Err(ErrorKind::VerifyError(VerifyError::InvalidExecutionGap).into());
		}

		if payload_execution_gap > system_meta.max_execution_gap {
			return Err(ErrorKind::VerifyError(VerifyError::InvalidExecutionGap).into());
		}

		// execution number of the confirmed block
		let confirmed_execution_number =
			confirmed_header.number - confirmed_header.payload_execution_gap as u64;
		if execution_number < confirmed_execution_number {
			return Err(ErrorKind::VerifyError(VerifyError::InvalidExecutionGap).into());
		}

		// execution number of current state
		let current_execution_number = current_state.executed_number;
		if execution_number > current_execution_number {
			return Err(ErrorKind::VerifyError(VerifyError::ShouldWait).into());
		}

		Ok(())
	}

	/// Return verified txs (meta_txs, payload_txs)
	fn verify_body(
		&self,
		meta_txs: Vec<Transaction>,
		payload_txs: Vec<Transaction>,
	) -> CommonResult<(Vec<Arc<FullTransaction>>, Vec<Arc<FullTransaction>>)> {
		let get_verified_txs = |txs: Vec<Transaction>| -> CommonResult<Vec<Arc<FullTransaction>>> {
			let mut set = HashSet::new();
			let mut result = Vec::with_capacity(txs.len());
			for tx in txs {
				let tx_hash = self.support.hash_transaction(&tx)?;
				self.verify_transaction(&tx_hash, &tx, &mut set)?;
				let tx = Arc::new(FullTransaction { tx_hash, tx });
				result.push(tx);
			}
			Ok(result)
		};

		let meta_txs = get_verified_txs(meta_txs)?;

		let payload_txs = get_verified_txs(payload_txs)?;

		Ok((meta_txs, payload_txs))
	}

	/// return commit block params
	fn verify_header(
		&self,
		block_hash: &Hash,
		number: BlockNumber,
		timestamp: u64,
		execution_number: BlockNumber,
		meta_txs: Vec<Arc<FullTransaction>>,
		payload_txs: Vec<Arc<FullTransaction>>,
	) -> CommonResult<ChainCommitBlockParams> {
		let build_block_params = BuildBlockParams {
			number,
			timestamp,
			meta_txs,
			payload_txs,
			execution_number,
		};

		let commit_block_params = self.support.build_block(build_block_params)?;

		if &commit_block_params.block_hash != block_hash {
			let msg = format!(
				"Invalid block_hash: {:?}, expected: {:?}",
				block_hash, commit_block_params.block_hash
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
			.validate_transaction(tx_hash, &tx, true)
			.or_else_catch::<node_chain::errors::ErrorKind, _>(|e| match e {
				node_chain::errors::ErrorKind::ValidateTxError(e) => Some(Err(
					ErrorKind::VerifyError(VerifyError::InvalidTx(e.clone())).into(),
				)),
				_ => None,
			})?;

		Ok(())
	}
}
