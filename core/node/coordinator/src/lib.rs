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

#![allow(clippy::single_match)]
#![allow(clippy::type_complexity)]

use std::sync::Arc;

pub use node_network::{
	ed25519, Keypair, LinkedHashMap, Multiaddr, Network, NetworkConfig, NetworkInMessage,
	NetworkState, OpenedPeer, PeerId, Protocol, UnopenedPeer,
};
use primitives::codec::Encode;
use primitives::errors::CommonResult;

use crate::protocol::{Handshake, ProtocolMessage};
use crate::stream::{start, CoordinatorStream};
use crate::support::CoordinatorSupport;
use futures::channel::mpsc::{unbounded, UnboundedSender};
use node_network::HandshakeBuilder;
use parking_lot::RwLock;
use primitives::{BlockNumber, Hash};

mod errors;
mod peer_report;
mod protocol;
mod stream;
pub mod support;
mod sync;
mod verifier;

pub struct CoordinatorConfig {
	pub network_config: NetworkConfig,
}

pub enum CoordinatorInMessage {
	Network(NetworkInMessage),
}

pub struct Coordinator<S>
where
	S: CoordinatorSupport,
{
	#[allow(dead_code)]
	network: Arc<Network>,
	#[allow(dead_code)]
	support: Arc<S>,
	coordinator_tx: UnboundedSender<CoordinatorInMessage>,
}

impl<S> Coordinator<S>
where
	S: CoordinatorSupport,
{
	pub fn new(config: CoordinatorConfig, support: Arc<S>) -> CommonResult<Self> {
		let genesis_hash = support
			.get_block_hash(&0)?
			.ok_or_else(|| errors::ErrorKind::Data("Missing genesis block".to_string()))?;

		let confirmed_number = support
			.get_confirmed_number()?
			.ok_or_else(|| errors::ErrorKind::Data("Missing confirmed number".to_string()))?;

		let confirmed_hash = support.get_block_hash(&confirmed_number)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing block hash: number: {}", confirmed_number))
		})?;

		let handshake_builder = Arc::new(DefaultHandshakeBuilder {
			genesis_hash,
			confirmed: RwLock::new((confirmed_number, confirmed_hash)),
		});

		let mut network_config = config.network_config;
		network_config.handshake_builder = Some(handshake_builder.clone());

		let network = Network::new(network_config)?;

		let peer_manager_tx = network.peer_manager_tx();
		let network_tx = network.network_tx();
		let network_rx = network.network_rx().expect("Coordinator is the only taker");

		let chain_rx = support.chain_rx().expect("Coordinator is the only taker");
		let txpool_rx = support.txpool_rx().expect("Coordinator is the only taker");

		let (in_tx, in_rx) = unbounded();

		let stream = CoordinatorStream::new(
			handshake_builder,
			chain_rx,
			txpool_rx,
			peer_manager_tx,
			network_tx,
			network_rx,
			in_rx,
			support.clone(),
		)?;

		tokio::spawn(start(stream));

		let coordinator = Coordinator {
			network: Arc::new(network),
			support,
			coordinator_tx: in_tx,
		};

		Ok(coordinator)
	}

	pub fn coordinator_tx(&self) -> UnboundedSender<CoordinatorInMessage> {
		self.coordinator_tx.clone()
	}
}

pub struct DefaultHandshakeBuilder {
	genesis_hash: Hash,
	confirmed: RwLock<(BlockNumber, Hash)>,
}

impl HandshakeBuilder for DefaultHandshakeBuilder {
	fn build(&self, nonce: u64) -> Vec<u8> {
		let (confirmed_number, confirmed_hash) = (*self.confirmed.read()).clone();
		ProtocolMessage::Handshake(Handshake {
			genesis_hash: self.genesis_hash.clone(),
			confirmed_number,
			confirmed_hash,
			nonce,
		})
		.encode()
	}
}
