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

//! Consensus

use std::sync::Arc;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};

use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{Consensus as ConsensusT, ConsensusInMessage, ConsensusOutMessage};
use node_consensus_poa::Poa;
pub use node_consensus_poa::PoaConfig;
use node_consensus_primitives::{CONSENSUS_POA, CONSENSUS_RAFT};
use node_consensus_raft::Raft;
pub use node_consensus_raft::RaftConfig;
use primitives::errors::CommonResult;
use primitives::{Header, Proof};

pub struct Consensus<S>
where
	S: ConsensusSupport,
{
	dispatcher: Dispatcher<S>,
}

pub struct ConsensusConfig {
	pub poa: Option<PoaConfig>,
	pub raft: Option<RaftConfig>,
}

enum Dispatcher<S>
where
	S: ConsensusSupport,
{
	Poa(Poa<S>),
	Raft(Raft<S>),
}

impl<S> Consensus<S>
where
	S: ConsensusSupport,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let dispatcher = Dispatcher::new(config, support)?;

		let consensus = Consensus { dispatcher };

		Ok(consensus)
	}

	pub fn verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()> {
		self.dispatcher.verify_proof(header, proof)
	}

	pub fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		self.dispatcher.in_message_tx()
	}

	pub fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		self.dispatcher.out_message_rx()
	}
}

impl<S> ConsensusT for Dispatcher<S>
where
	S: ConsensusSupport,
{
	type Config = ConsensusConfig;
	type Support = S;

	fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let system_meta = &support.get_current_state().system_meta;
		let consensus = system_meta.consensus.as_str();

		let dispatcher = match consensus {
			CONSENSUS_POA => {
				let config = config.poa.ok_or_else(|| {
					node_consensus_base::errors::ErrorKind::Data("Missing poa config".to_string())
				})?;
				Dispatcher::Poa(Poa::new(config, support)?)
			}
			CONSENSUS_RAFT => {
				let config = config.raft.ok_or_else(|| {
					node_consensus_base::errors::ErrorKind::Data("Missing raft config".to_string())
				})?;
				Dispatcher::Raft(Raft::new(config, support)?)
			}
			other => {
				panic!("Unknown consensus: {}", other);
			}
		};
		Ok(dispatcher)
	}

	fn verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()> {
		match self {
			Dispatcher::Poa(c) => c.verify_proof(header, proof),
			Dispatcher::Raft(c) => c.verify_proof(header, proof),
		}
	}

	fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		match self {
			Dispatcher::Poa(c) => c.in_message_tx(),
			Dispatcher::Raft(c) => c.in_message_tx(),
		}
	}

	fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		match self {
			Dispatcher::Poa(c) => c.out_message_rx(),
			Dispatcher::Raft(c) => c.out_message_rx(),
		}
	}
}
