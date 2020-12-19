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

use std::num::NonZeroUsize;

use futures_codec::BytesMut;
use libp2p::core::connection::ConnectionLimits;
use libp2p::core::transport::upgrade;
use libp2p::core::{ConnectedPoint, Multiaddr};
use libp2p::identity::Keypair;
use libp2p::swarm::{AddressScore, SwarmBuilder};
use libp2p::{PeerId, Swarm};
use linked_hash_map::LinkedHashMap;
use log::info;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use node_peer_manager::{PeerManager, PeerManagerConfig};
use primitives::errors::CommonResult;

use crate::behaviour::{Behaviour, BehaviourConfig};

use crate::stream::{start, NetworkStream};
pub use node_peer_manager::InMessage as PMInMessage;

mod behaviour;
mod discovery;
mod errors;
mod protocol;
mod stream;
mod transport;

pub struct NetworkConfig {
	pub max_in_peers: u32,
	pub max_out_peers: u32,
	pub listen_addresses: LinkedHashMap<Multiaddr, ()>,
	pub external_addresses: LinkedHashMap<Multiaddr, ()>,
	pub bootnodes: LinkedHashMap<(PeerId, Multiaddr), ()>,
	pub reserved_nodes: LinkedHashMap<(PeerId, Multiaddr), ()>,
	pub reserved_only: bool,
	pub agent_version: String,
	pub local_key_pair: Keypair,
}

#[allow(dead_code)]
pub enum NetworkInMessage {
	SendMessage { peer_id: PeerId, message: Vec<u8> },
}

#[allow(dead_code)]
pub enum NetWorkOutMessage {
	ProtocolOpen {
		peer_id: PeerId,
		connected_point: ConnectedPoint,
	},
	ProtocolClose {
		peer_id: PeerId,
		connected_point: ConnectedPoint,
	},
	Message {
		message: BytesMut,
	},
}

#[allow(dead_code)]
pub struct Network {
	peer_manager_tx: UnboundedSender<PMInMessage>,
	network_tx: UnboundedSender<NetworkInMessage>,
	network_rx: Option<UnboundedReceiver<NetWorkOutMessage>>,
}

impl Network {
	pub fn new(config: NetworkConfig) -> CommonResult<Self> {
		let mut known_addresses = Vec::new();
		let mut bootnodes = LinkedHashMap::new();
		let mut reserved_nodes = LinkedHashMap::new();
		for ((peer_id, address), _) in config.bootnodes {
			bootnodes.insert(peer_id.clone(), ());
			known_addresses.push((peer_id, address));
		}
		for ((peer_id, address), _) in config.reserved_nodes {
			reserved_nodes.insert(peer_id.clone(), ());
			known_addresses.push((peer_id, address));
		}

		// peer manager
		let peer_manager_config = PeerManagerConfig {
			max_in_peers: config.max_in_peers,
			max_out_peers: config.max_out_peers,
			bootnodes,
			reserved: reserved_nodes,
			reserved_only: config.reserved_only,
		};
		let peer_manager = PeerManager::new(peer_manager_config);
		let peer_manager_tx = peer_manager.tx();

		// behaviour
		let local_public_key = config.local_key_pair.public();
		let local_peer_id = local_public_key.clone().into_peer_id();
		let discovery_max_connections = Some(config.max_in_peers + config.max_out_peers);
		let behaviour_config = BehaviourConfig {
			agent_version: config.agent_version,
			local_public_key,
			known_addresses,
			discovery_max_connections,
		};

		let behaviour = Behaviour::new(behaviour_config, peer_manager);
		let (transport, bandwidth) = transport::build_transport(config.local_key_pair)?;

		let builder = SwarmBuilder::new(transport, behaviour, local_peer_id)
			.connection_limits(
				ConnectionLimits::default()
					.with_max_established_per_peer(Some(1u32))
					.with_max_established_incoming(Some(1u32)),
			)
			.substream_upgrade_protocol_override(upgrade::Version::V1Lazy)
			.notify_handler_buffer_size(NonZeroUsize::new(32).expect("qed"))
			.connection_event_buffer_size(1024);

		let mut swarm = builder.build();

		for (address, _) in config.listen_addresses {
			Swarm::listen_on(&mut swarm, address.clone())
				.map_err(|e| errors::ErrorKind::Transport(format!("{}", e)))?;
		}

		for (address, _) in config.external_addresses {
			Swarm::add_external_address(&mut swarm, address.clone(), AddressScore::Infinite);
		}

		let (in_tx, in_rx) = unbounded_channel();
		let (out_tx, out_rx) = unbounded_channel();

		let stream = NetworkStream {
			swarm,
			bandwidth,
			in_rx,
			out_tx,
		};
		tokio::spawn(start(stream));

		let network = Network {
			peer_manager_tx,
			network_tx: in_tx,
			network_rx: Some(out_rx),
		};

		info!("Initializing network");
		Ok(network)
	}

	pub fn peer_manager_tx(&self) -> UnboundedSender<PMInMessage> {
		self.peer_manager_tx.clone()
	}

	pub fn network_tx(&self) -> UnboundedSender<NetworkInMessage> {
		self.network_tx.clone()
	}

	pub fn network_rx(&mut self) -> Option<UnboundedReceiver<NetWorkOutMessage>> {
		self.network_rx.take()
	}
}
