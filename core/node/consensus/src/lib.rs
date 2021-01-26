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
use node_consensus_base::{
	Consensus as ConsensusT, ConsensusConfig, ConsensusInMessage, ConsensusOutMessage,
};
use node_consensus_poa::Poa;
use node_consensus_primitives::CONSENSUS_POA;
use primitives::errors::CommonResult;
use primitives::{Header, Proof};

pub struct Consensus<S>
where
	S: ConsensusSupport,
{
	dispatcher: Dispatcher<S>,
}

enum Dispatcher<S>
where
	S: ConsensusSupport,
{
	Poa(Poa<S>),
}

impl<S> Consensus<S>
where
	S: ConsensusSupport,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let system_meta = &support.get_current_state().system_meta;
		let consensus = system_meta.consensus.as_str();

		let dispatcher = match consensus {
			CONSENSUS_POA => {
				let poa = Poa::new(config, support)?;
				Dispatcher::Poa(poa)
			}
			other => {
				panic!("Unknown consensus: {}", other);
			}
		};

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
	fn verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()> {
		match self {
			Dispatcher::Poa(poa) => poa.verify_proof(header, proof),
		}
	}

	fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		match self {
			Dispatcher::Poa(poa) => poa.in_message_tx(),
		}
	}

	fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		match self {
			Dispatcher::Poa(poa) => poa.out_message_rx(),
		}
	}
}
