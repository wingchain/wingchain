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

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
pub use node_network::PeerId;
use primitives::errors::CommonResult;
use primitives::{BlockNumber, Hash, Header, Proof};
use serde_json::Value;
use std::sync::Arc;

pub mod errors;
pub mod scheduler;
pub mod support;

pub trait Consensus: Sized {
	type Config;
	type Support;
	fn new(config: Self::Config, support: Arc<Self::Support>) -> CommonResult<Self>;
	fn verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()>;
	fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage>;
	fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>>;
}

pub enum ConsensusInMessage {
	NetworkProtocolOpen {
		peer_id: PeerId,
		local_nonce: u64,
		remote_nonce: u64,
	},
	NetworkProtocolClose {
		peer_id: PeerId,
	},
	NetworkMessage {
		peer_id: PeerId,
		message: Vec<u8>,
	},
	BlockCommitted {
		number: BlockNumber,
		block_hash: Hash,
	},
	Generate,
	GetConsensusState {
		tx: oneshot::Sender<Value>,
	},
}

pub enum ConsensusOutMessage {
	NetworkMessage { peer_id: PeerId, message: Vec<u8> },
}
