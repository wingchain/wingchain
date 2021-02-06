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

use node_chain::DBTransaction;
use node_consensus_base::support::ConsensusSupport;
use primitives::errors::CommonResult;
use primitives::{codec, Address};

use crate::proof::Proof;
use crate::protocol::{Entry, EntryData, Proposal};
use parking_lot::RwLock;
use std::collections::btree_map::Range;
use std::collections::BTreeMap;
use std::ops::RangeBounds;
use std::sync::Arc;

const DB_KEY_CURRENT_TERM: &[u8] = b"current_term";
const DB_KEY_CURRENT_VOTED_FOR: &[u8] = b"current_voted_for";
const DB_KEY_COMMIT_LOG_INDEX: &[u8] = b"commit_log_index";
const DB_KEY_LOGS: &[u8] = b"logs";
const DB_KEY_PROPOSAL: &[u8] = b"proposal";

pub struct Storage<S>
where
	S: ConsensusSupport,
{
	base_log_index: RwLock<u64>,
	base_log_term: RwLock<u64>,
	current_term: RwLock<u64>,
	current_voted_for: RwLock<Option<Address>>,
	commit_log_index: RwLock<u64>,
	logs: RwLock<BTreeMap<u64, Entry>>,
	proposal: RwLock<Option<Proposal>>,
	support: Arc<S>,
}

impl<S> Storage<S>
where
	S: ConsensusSupport,
{
	pub fn new(support: Arc<S>) -> CommonResult<Self> {
		let this = Self {
			base_log_index: RwLock::new(0),
			base_log_term: RwLock::new(0),
			current_term: RwLock::new(0),
			current_voted_for: RwLock::new(None),
			commit_log_index: RwLock::new(0),
			logs: RwLock::new(Default::default()),
			proposal: RwLock::new(None),
			support,
		};
		this.refresh()?;
		Ok(this)
	}

	pub fn refresh(&self) -> CommonResult<()> {
		// init base_log_index, base_log_term
		let proof = self.get_proof()?;
		let (base_log_index, base_log_term) = match proof {
			Some(proof) => (proof.log_index, proof.log_term),
			None => (0, 0),
		};

		// init current_term, current_voted_for
		// and fix if needed
		let mut current_term: u64 = self
			.support
			.get_consensus_data(DB_KEY_CURRENT_TERM)?
			.unwrap_or_default();

		let mut current_voted_for: Option<Address> = self
			.support
			.get_consensus_data(DB_KEY_CURRENT_VOTED_FOR)?
			.unwrap_or_default();

		if current_term < base_log_term {
			current_term = base_log_term;
			current_voted_for = None;
			self.commit_consensus_data(|transaction| {
				self.support.update_consensus_data(
					transaction,
					DB_KEY_CURRENT_TERM,
					current_term,
				)?;
				self.support.update_consensus_data(
					transaction,
					DB_KEY_CURRENT_VOTED_FOR,
					&current_voted_for,
				)?;
				Ok(())
			})?;
		}

		// init commit_log_index
		// and fix if needed
		let mut commit_log_index: u64 = self
			.support
			.get_consensus_data(DB_KEY_COMMIT_LOG_INDEX)?
			.unwrap_or_default();

		if commit_log_index < base_log_index {
			commit_log_index = base_log_index;
			self.commit_consensus_data(|transaction| {
				self.support.update_consensus_data(
					transaction,
					DB_KEY_COMMIT_LOG_INDEX,
					commit_log_index,
				)?;
				Ok(())
			})?;
		}

		// init logs
		// and fix if needed
		let mut logs = {
			let logs: Vec<(u64, Entry)> = self
				.support
				.get_consensus_data(DB_KEY_LOGS)?
				.unwrap_or_default();
			logs.into_iter().collect::<BTreeMap<_, _>>()
		};
		let to_remove_key = logs
			.range(..=base_log_index)
			.map(|(k, _)| *k)
			.collect::<Vec<_>>();
		for k in &to_remove_key {
			logs.remove(k);
		}
		if !to_remove_key.is_empty() {
			let logs_vec = logs.iter().collect::<Vec<_>>();
			self.commit_consensus_data(|transaction| {
				self.support
					.update_consensus_data(transaction, DB_KEY_LOGS, &logs_vec)?;
				Ok(())
			})?;
		}

		// init proposal
		// and fix if needed
		let mut proposal: Option<Proposal> = self
			.support
			.get_consensus_data(DB_KEY_PROPOSAL)?
			.unwrap_or_default();
		let contained_in_logs = logs.iter().any(|(_, v)| match &v.data {
			EntryData::Proposal { block_hash, .. } => {
				Some(block_hash) == proposal.as_ref().map(|p| &p.block_hash)
			}
			_ => false,
		});
		if !contained_in_logs {
			proposal = None;
			self.commit_consensus_data(|transaction| {
				self.support
					.update_consensus_data(transaction, DB_KEY_PROPOSAL, &proposal)?;
				Ok(())
			})?;
		}

		(*self.base_log_index.write()) = base_log_index;
		(*self.base_log_term.write()) = base_log_term;
		(*self.current_term.write()) = current_term;
		(*self.current_voted_for.write()) = current_voted_for;
		(*self.commit_log_index.write()) = commit_log_index;
		(*self.logs.write()) = logs;
		(*self.proposal.write()) = proposal;

		Ok(())
	}

	pub fn get_base_log_index_term(&self) -> (u64, u64) {
		(*self.base_log_index.read(), *self.base_log_term.read())
	}

	pub fn get_last_log_index_term(&self) -> (u64, u64) {
		match self.logs.read().iter().last() {
			Some((_k, v)) => (v.index, v.term),
			None => (*self.base_log_index.read(), *self.base_log_term.read()),
		}
	}

	pub fn get_current_term(&self) -> u64 {
		*self.current_term.read()
	}

	pub fn update_current_term(&self, current_term: u64) -> CommonResult<()> {
		self.commit_consensus_data(|transaction| {
			self.support
				.update_consensus_data(transaction, DB_KEY_CURRENT_TERM, current_term)?;
			Ok(())
		})?;
		*self.current_term.write() = current_term;
		Ok(())
	}

	pub fn get_current_voted_for(&self) -> Option<Address> {
		(*self.current_voted_for.read()).clone()
	}

	pub fn update_current_voted_for(&self, current_voted_for: Option<Address>) -> CommonResult<()> {
		self.commit_consensus_data(|transaction| {
			self.support.update_consensus_data(
				transaction,
				DB_KEY_CURRENT_VOTED_FOR,
				&current_voted_for,
			)?;
			Ok(())
		})?;
		*self.current_voted_for.write() = current_voted_for;
		Ok(())
	}

	pub fn get_commit_log_index(&self) -> u64 {
		*self.commit_log_index.read()
	}

	pub fn update_commit_log_index(&self, commit_log_index: u64) -> CommonResult<()> {
		self.commit_consensus_data(|transaction| {
			self.support.update_consensus_data(
				transaction,
				DB_KEY_COMMIT_LOG_INDEX,
				&commit_log_index,
			)?;
			Ok(())
		})?;
		*self.commit_log_index.write() = commit_log_index;
		Ok(())
	}

	pub fn get_log_entries<R>(&self, range: R) -> Vec<Entry>
	where
		R: RangeBounds<u64>,
	{
		self.get_log_entries_using(range, |x| x.map(|(_, v)| v.clone()).collect())
	}

	pub fn get_log_entries_using<R, T, F>(&self, range: R, using: F) -> T
	where
		F: Fn(Range<u64, Entry>) -> T,
		R: RangeBounds<u64>,
	{
		using(self.logs.read().range(range))
	}

	pub fn append_log_entries(&self, entry: Vec<Entry>) -> CommonResult<()> {
		(*self.logs.write()).extend(entry.into_iter().map(|x| (x.index, x)));
		let logs_vec = self
			.logs
			.read()
			.iter()
			.map(|(k, v)| (*k, v.clone()))
			.collect::<Vec<_>>();
		self.commit_consensus_data(|transaction| {
			self.support
				.update_consensus_data(transaction, DB_KEY_LOGS, &logs_vec)?;
			Ok(())
		})?;
		Ok(())
	}

	pub fn delete_log_entries<R>(&self, range: R) -> CommonResult<()>
	where
		R: RangeBounds<u64>,
	{
		let to_remove_key = self
			.logs
			.read()
			.range(range)
			.map(|(k, _)| *k)
			.collect::<Vec<_>>();

		let mut guard = self.logs.write();
		for key in &to_remove_key {
			guard.remove(key);
		}
		drop(guard);
		let logs_vec = self
			.logs
			.read()
			.iter()
			.map(|(k, v)| (*k, v.clone()))
			.collect::<Vec<_>>();
		self.commit_consensus_data(|transaction| {
			self.support
				.update_consensus_data(transaction, DB_KEY_LOGS, &logs_vec)?;
			Ok(())
		})?;

		Ok(())
	}

	pub fn get_proposal(&self) -> Option<Proposal> {
		self.get_proposal_using(|x| x.clone())
	}

	pub fn get_proposal_using<T, F: Fn(&Option<Proposal>) -> T>(&self, using: F) -> T {
		using(&*self.proposal.read())
	}

	pub fn update_proposal(&self, proposal: Option<Proposal>) -> CommonResult<()> {
		self.commit_consensus_data(|transaction| {
			self.support
				.update_consensus_data(transaction, DB_KEY_PROPOSAL, &proposal)?;
			Ok(())
		})?;
		*self.proposal.write() = proposal;
		Ok(())
	}

	fn get_proof(&self) -> CommonResult<Option<Proof>> {
		let current_state = self.support.get_current_state();
		let confirmed_number = current_state.confirmed_number;
		let proof = match confirmed_number {
			0 => None,
			_ => {
				let confirmed_block_hash = &current_state.confirmed_block_hash;
				let proof = self
					.support
					.get_proof(confirmed_block_hash)?
					.ok_or_else(|| {
						node_consensus_base::errors::ErrorKind::Data(format!(
							"Missing proof: block_hash: {}",
							confirmed_block_hash
						))
					})?;
				let data = proof.data;
				let proof: Proof = codec::decode(&mut &data[..]).map_err(|_| {
					node_consensus_base::errors::ErrorKind::Data("Decode proof error".to_string())
				})?;
				Some(proof)
			}
		};
		Ok(proof)
	}

	fn commit_consensus_data<OP: Fn(&mut DBTransaction) -> CommonResult<()>>(
		&self,
		op: OP,
	) -> CommonResult<()> {
		let mut transaction = DBTransaction::new();
		op(&mut transaction)?;
		self.support.commit_consensus_data(transaction)
	}
}
