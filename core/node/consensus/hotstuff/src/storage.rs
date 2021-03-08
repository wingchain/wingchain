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

use std::collections::btree_map::Range;
use std::collections::{BTreeMap, HashMap};
use std::ops::RangeBounds;
use std::sync::Arc;

use parking_lot::RwLock;

use node_chain::DBTransaction;
use node_consensus_base::support::ConsensusSupport;
use primitives::errors::CommonResult;
use primitives::{codec, Address, Hash};

use crate::proof::Proof;
use crate::protocol::{Node, QC};

const DB_KEY_PREPARE_QC: &[u8] = b"prepare_qc";
const DB_KEY_LOCKED_QC: &[u8] = b"locked_qc";
const DB_KEY_NODES: &[u8] = b"nodes";

pub struct Storage<S>
where
	S: ConsensusSupport,
{
	base_qc: RwLock<Option<QC>>,
	base_block_hash: RwLock<Hash>,
	prepare_qc: RwLock<Option<QC>>,
	locked_qc: RwLock<Option<QC>>,
	nodes: RwLock<HashMap<Hash, Node>>,
	support: Arc<S>,
}

impl<S> Storage<S>
where
	S: ConsensusSupport,
{
	pub fn new(support: Arc<S>) -> CommonResult<Self> {
		let this = Self {
			base_qc: RwLock::new(None),
			base_block_hash: RwLock::new(Hash(Default::default())),
			prepare_qc: RwLock::new(None),
			locked_qc: RwLock::new(None),
			nodes: RwLock::new(HashMap::new()),
			support,
		};
		this.refresh()?;
		Ok(this)
	}

	pub fn refresh(&self) -> CommonResult<()> {
		let proof = self.get_proof()?;

		let base_qc = match proof {
			Some(proof) => Some(proof.commit_qc),
			None => None,
		};

		let current_state = self.support.get_current_state();
		let base_block_hash = current_state.confirmed_block_hash.clone();

		// init prepare qc
		// and fix if needed
		let mut prepare_qc: Option<QC> = self
			.support
			.get_consensus_data(DB_KEY_PREPARE_QC)?
			.unwrap_or_default();

		if let (Some(pqc), Some(bqc)) = (&prepare_qc, &base_qc) {
			if pqc.view <= bqc.view {
				prepare_qc = None;
				self.commit_consensus_data(|transaction| {
					self.support.update_consensus_data(
						transaction,
						DB_KEY_PREPARE_QC,
						&prepare_qc,
					)?;
					Ok(())
				})?;
			}
		}

		// init locked qc
		// and fix if needed
		let mut locked_qc: Option<QC> = self
			.support
			.get_consensus_data(DB_KEY_LOCKED_QC)?
			.unwrap_or_default();

		if let (Some(lqc), Some(bqc)) = (&locked_qc, &base_qc) {
			if lqc.view <= bqc.view {
				locked_qc = None;
				self.commit_consensus_data(|transaction| {
					self.support.update_consensus_data(
						transaction,
						DB_KEY_LOCKED_QC,
						&locked_qc,
					)?;
					Ok(())
				})?;
			}
		}

		// init nodes
		// and fix if needed
		let mut nodes = {
			let nodes: Vec<(Hash, Node)> = self
				.support
				.get_consensus_data(DB_KEY_NODES)?
				.unwrap_or_default();
			nodes.into_iter().collect::<HashMap<_, _>>()
		};

		let to_remove_key = nodes
			.iter()
			.filter(|(_, v)| v.parent != base_block_hash)
			.map(|(k, _)| k.clone())
			.collect::<Vec<_>>();
		for k in &to_remove_key {
			nodes.remove(k);
		}
		if !to_remove_key.is_empty() {
			let nodes_vec = nodes.iter().collect::<Vec<_>>();
			self.commit_consensus_data(|transaction| {
				self.support
					.update_consensus_data(transaction, DB_KEY_NODES, &nodes_vec)?;
				Ok(())
			})?;
		}

		(*self.base_qc.write()) = base_qc;
		(*self.base_block_hash.write()) = base_block_hash;
		(*self.nodes.write()) = nodes;

		Ok(())
	}

	pub fn get_base_qc(&self) -> Option<QC> {
		self.base_qc.read().clone()
	}

	pub fn get_prepare_qc(&self) -> Option<QC> {
		self.prepare_qc.read().clone()
	}

	pub fn get_lock_qc(&self) -> Option<QC> {
		self.locked_qc.read().clone()
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
