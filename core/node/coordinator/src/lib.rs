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

use std::sync::Arc;

pub use node_network::{
	ed25519, Keypair, LinkedHashMap, Multiaddr, Network, NetworkConfig, PeerId, Protocol,
};
use primitives::codec::Encode;
use primitives::errors::CommonResult;

use crate::protocol::{Handshake, ProtocolMessage};
use crate::stream::{start, CoordinatorStream};
use crate::support::CoordinatorSupport;

mod errors;
mod protocol;
mod stream;
pub mod support;
mod sync;

pub struct CoordinatorConfig {
	pub network_config: NetworkConfig,
}

pub struct Coordinator<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	#[allow(dead_code)]
	network: Arc<Network>,
	#[allow(dead_code)]
	support: Arc<S>,
}

impl<S> Coordinator<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	pub fn new(config: CoordinatorConfig, support: Arc<S>) -> CommonResult<Self> {
		let genesis_hash = support
			.get_block_hash(&0)?
			.ok_or(errors::ErrorKind::Data("Missing genesis block".to_string()))?;

		let handshake = ProtocolMessage::Handshake(Handshake {
			genesis_hash: genesis_hash.clone(),
		})
		.encode();

		let mut network_config = config.network_config;
		network_config.handshake = handshake;

		let network = Network::new(network_config)?;

		let peer_manager_tx = network.peer_manager_tx();
		let network_tx = network.network_tx();
		let network_rx = network.network_rx().expect("Coordinator is the only taker");

		let chain_rx = support.chain_rx().expect("Coordinator is the only taker");

		let stream = CoordinatorStream::new(
			genesis_hash,
			chain_rx,
			peer_manager_tx,
			network_tx,
			network_rx,
			support.clone(),
		)?;

		tokio::spawn(start(stream));

		let coordinator = Coordinator {
			network: Arc::new(network),
			support,
		};

		Ok(coordinator)
	}
}
