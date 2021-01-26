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

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use log::error;
use parking_lot::RwLock;

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
	in_tx: UnboundedSender<ConsensusInMessage>,
	out_rx: RwLock<Option<UnboundedReceiver<ConsensusOutMessage>>>,
	dispatcher: Arc<Dispatcher<S>>,
}

enum Dispatcher<S>
where
	S: ConsensusSupport,
{
	Poa(Poa<S>),
}

struct ConsensusStream<S>
where
	S: ConsensusSupport,
{
	in_rx: UnboundedReceiver<ConsensusInMessage>,
	dispatcher: Arc<Dispatcher<S>>,
}

impl<S> Consensus<S>
where
	S: ConsensusSupport,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let system_meta = &support.get_current_state().system_meta;
		let consensus = system_meta.consensus.as_str();

		let (in_tx, in_rx) = unbounded();
		let (out_tx, out_rx) = unbounded();

		let dispatcher = match consensus {
			CONSENSUS_POA => {
				let poa = Poa::new(config, out_tx, support)?;
				Dispatcher::Poa(poa)
			}
			other => {
				panic!("Unknown consensus: {}", other);
			}
		};
		let dispatcher = Arc::new(dispatcher);

		let stream = ConsensusStream {
			in_rx,
			dispatcher: dispatcher.clone(),
		};
		tokio::spawn(start(stream));

		let consensus = Consensus {
			in_tx,
			out_rx: RwLock::new(Some(out_rx)),
			dispatcher,
		};

		Ok(consensus)
	}

	pub fn generate(&self) -> CommonResult<()> {
		self.dispatcher.generate()
	}

	pub fn verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()> {
		self.dispatcher.verify_proof(header, proof)
	}

	pub fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		self.in_tx.clone()
	}

	pub fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		self.out_rx.write().take()
	}
}

impl<S> ConsensusT for Dispatcher<S>
where
	S: ConsensusSupport,
{
	fn generate(&self) -> CommonResult<()> {
		match self {
			Dispatcher::Poa(poa) => poa.generate(),
		}
	}

	fn verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()> {
		match self {
			Dispatcher::Poa(poa) => poa.verify_proof(header, proof),
		}
	}

	fn on_in_message(&self, in_message: ConsensusInMessage) -> CommonResult<()> {
		match self {
			Dispatcher::Poa(poa) => poa.on_in_message(in_message),
		}
	}
}

#[allow(clippy::while_let_loop)]
async fn start<S>(mut stream: ConsensusStream<S>)
where
	S: ConsensusSupport,
{
	loop {
		match stream.in_rx.next().await {
			Some(in_message) => {
				stream
					.dispatcher
					.on_in_message(in_message)
					.unwrap_or_else(|e| error!("Consensus handle in message error: {}", e));
			}
			None => break,
		}
	}
}
