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

//! Raft consensus

use std::sync::Arc;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use log::info;
use parking_lot::RwLock;

use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{
	Consensus as ConsensusT, ConsensusConfig, ConsensusInMessage, ConsensusOutMessage,
};
use node_consensus_primitives::CONSENSUS_RAFT;
use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::errors::CommonResult;
use primitives::{BlockNumber, Header};

use crate::stream::RaftStream;

pub mod errors;
mod network;
mod proof;
mod protocol;
mod state;
mod storage;
mod stream;

pub struct Raft<S>
where
	S: ConsensusSupport,
{
	#[allow(dead_code)]
	support: Arc<S>,
	in_tx: UnboundedSender<ConsensusInMessage>,
	out_rx: RwLock<Option<UnboundedReceiver<ConsensusOutMessage>>>,
}

impl<S> Raft<S>
where
	S: ConsensusSupport,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let raft_meta = get_raft_meta(&support, &0)?;

		let secret_key = config.secret_key;

		let (in_tx, in_rx) = unbounded();
		let (out_tx, out_rx) = unbounded();

		RaftStream::spawn(support.clone(), raft_meta, secret_key, out_tx, in_rx)?;

		info!("Initializing consensus raft");

		let raft = Raft {
			support,
			in_tx,
			out_rx: RwLock::new(Some(out_rx)),
		};

		Ok(raft)
	}
}

impl<S> ConsensusT for Raft<S>
where
	S: ConsensusSupport,
{
	fn verify_proof(&self, _header: &Header, proof: &primitives::Proof) -> CommonResult<()> {
		let name = &proof.name;
		if name != CONSENSUS_RAFT {
			return Err(
				node_consensus_base::errors::ErrorKind::VerifyProofError(format!(
					"Unexpected consensus: {}",
					name
				))
				.into(),
			);
		}
		unimplemented!()
	}

	fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		self.in_tx.clone()
	}

	fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		self.out_rx.write().take()
	}
}

fn get_raft_meta<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<module::raft::Meta> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"raft".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}
