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

use std::collections::{HashSet, VecDeque};
use std::time::Duration;

use fnv::FnvHashMap;
use futures::task::{Context, Poll};
use futures_codec::BytesMut;
use libp2p::core::{ConnectedPoint, Multiaddr};
use libp2p::identify::{Identify, IdentifyEvent, IdentifyInfo};
use libp2p::identity::PublicKey;
use libp2p::ping::{Ping, PingEvent, PingSuccess};
use libp2p::swarm::{
	IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, NetworkBehaviourEventProcess,
	PollParameters, ProtocolsHandler,
};
use libp2p::{NetworkBehaviour, PeerId};
use log::{debug, trace};

use node_peer_manager::PeerManager;

use crate::discovery::{Discovery, DiscoveryConfig, DiscoveryOut};
use crate::protocol::{Protocol, ProtocolConfig, ProtocolOut};
use crate::HandshakeBuilder;
use std::sync::Arc;

const GLOBAL_PROTOCOL_VERSION: &str = "/wingchain/1.0.0";

pub struct BehaviourConfig {
	pub agent_version: String,
	pub local_public_key: PublicKey,
	pub known_addresses: Vec<(PeerId, Multiaddr)>,
	pub discovery_max_connections: Option<u32>,
	pub handshake_builder: Arc<dyn HandshakeBuilder>,
}

#[derive(Debug)]
pub enum BehaviourOut {
	ProtocolOpen {
		peer_id: PeerId,
		connected_point: ConnectedPoint,
		nonce: u64,
		handshake: Vec<u8>,
	},
	ProtocolClose {
		peer_id: PeerId,
		connected_point: ConnectedPoint,
	},
	Message {
		peer_id: PeerId,
		message: BytesMut,
	},
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourOut", poll_method = "poll")]
pub struct Behaviour {
	protocol: Protocol,
	ping: Ping,
	identify: Identify,
	discovery: Discovery,
	#[behaviour(ignore)]
	peers: FnvHashMap<PeerId, PeerInfo>,
	#[behaviour(ignore)]
	events: VecDeque<BehaviourOut>,
}

#[derive(Clone)]
pub struct PeerInfo {
	pub connected_point: ConnectedPoint,
	pub agent_version: Option<String>,
	pub latest_ping: Option<Duration>,
}

impl Behaviour {
	pub fn new(config: BehaviourConfig, peer_manager: PeerManager) -> Self {
		let ping = Ping::default();
		let identify = Identify::new(
			GLOBAL_PROTOCOL_VERSION.to_string(),
			config.agent_version,
			config.local_public_key.clone(),
		);
		let local_peer_id = config.local_public_key.clone().into_peer_id();
		let discovery_config = DiscoveryConfig {
			local_peer_id: local_peer_id.clone(),
			user_defined: config.known_addresses,
			max_connections: config.discovery_max_connections,
		};
		let discovery = Discovery::new(discovery_config);

		let protocol_config = ProtocolConfig {
			local_peer_id,
			handshake_builder: config.handshake_builder,
		};
		let protocol = Protocol::new(protocol_config, peer_manager);
		Self {
			protocol,
			ping,
			identify,
			discovery,
			peers: FnvHashMap::default(),
			events: VecDeque::new(),
		}
	}

	pub fn send_message(&mut self, peer_id: PeerId, message: Vec<u8>) {
		self.protocol.send_message(peer_id, message);
	}

	pub fn drop_peer(&mut self, peer_id: PeerId, delay: Option<Duration>) {
		self.protocol.drop_peer(peer_id, delay);
	}

	#[allow(clippy::mutable_key_type)]
	pub fn peers(&self) -> &FnvHashMap<PeerId, PeerInfo> {
		&self.peers
	}

	#[allow(clippy::mutable_key_type)]
	pub fn known_peers(&mut self) -> HashSet<PeerId> {
		self.discovery.known_peers()
	}

    fn poll(&mut self, _: &mut Context, _: &mut impl PollParameters) ->
    Poll<NetworkBehaviourAction<<<<Self as NetworkBehaviour>::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::InEvent,
	<Self as NetworkBehaviour>::OutEvent>>{
		if let Some(event) = self.events.pop_front() {
			return Poll::Ready(NetworkBehaviourAction::GenerateEvent(event));
		}

		Poll::Pending
	}
}

impl NetworkBehaviourEventProcess<ProtocolOut> for Behaviour {
	fn inject_event(&mut self, event: ProtocolOut) {
		match event {
			ProtocolOut::ProtocolOpen {
				peer_id,
				connected_point,
				nonce,
				handshake,
			} => {
				self.peers.insert(
					peer_id.clone(),
					PeerInfo {
						connected_point: connected_point.clone(),
						agent_version: None,
						latest_ping: None,
					},
				);
				self.events.push_back(BehaviourOut::ProtocolOpen {
					peer_id,
					connected_point,
					nonce,
					handshake,
				});
			}
			ProtocolOut::ProtocolClose {
				peer_id,
				connected_point,
			} => {
				self.peers.remove(&peer_id);
				self.events.push_back(BehaviourOut::ProtocolClose {
					peer_id,
					connected_point,
				});
			}
			ProtocolOut::Message { peer_id, message } => {
				self.events
					.push_back(BehaviourOut::Message { peer_id, message });
			}
		}
	}
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for Behaviour {
	fn inject_event(&mut self, event: IdentifyEvent) {
		match event {
			IdentifyEvent::Received { peer_id, info, .. } => {
				trace!("Identified {}: {:?}", peer_id, info);

				let IdentifyInfo {
					agent_version,
					mut listen_addrs,
					..
				} = info;
				if listen_addrs.len() > 32 {
					listen_addrs.truncate(30);
				}
				for addr in listen_addrs {
					self.discovery.add_address(&peer_id, addr);
				}
				self.protocol
					.add_discovered_peers(std::iter::once(peer_id.clone()));

				if let Some(peer_info) = self.peers.get_mut(&peer_id) {
					peer_info.agent_version = Some(agent_version);
				}
			}
			IdentifyEvent::Error { peer_id, error } => {
				debug!("Identify error with {} => {:?}", peer_id, error);
			}
			_ => (),
		}
	}
}

impl NetworkBehaviourEventProcess<PingEvent> for Behaviour {
	fn inject_event(&mut self, event: PingEvent) {
		if let PingEvent {
			peer: peer_id,
			result: Ok(PingSuccess::Ping { rtt }),
		} = event
		{
			trace!("Ping success with {}: {:?}", peer_id, rtt);

			if let Some(peer_info) = self.peers.get_mut(&peer_id) {
				peer_info.latest_ping = Some(rtt);
			}
		}
	}
}

impl NetworkBehaviourEventProcess<DiscoveryOut> for Behaviour {
	fn inject_event(&mut self, event: DiscoveryOut) {
		match event {
			DiscoveryOut::Discovered { peer_id } => {
				trace!("Discovered {}", peer_id);
				self.protocol.add_discovered_peers(std::iter::once(peer_id));
			}
		}
	}
}
