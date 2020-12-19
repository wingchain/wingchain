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

use std::cmp;
use std::io;

use futures::task::{Context, Poll};
use futures::FutureExt;
use libp2p::core::connection::{ConnectionId, ListenerId};
use libp2p::core::{ConnectedPoint, Multiaddr};
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{Kademlia, KademliaBucketInserts, KademliaConfig, KademliaEvent};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{
	IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters,
	ProtocolsHandler,
};
use libp2p::PeerId;
use log::{debug, info, trace};
use lru::LruCache;
use tokio::time::{delay_for, Delay, Duration};

pub struct DiscoveryConfig {
	pub local_peer_id: PeerId,
	pub user_defined: Vec<(PeerId, Multiaddr)>,
	pub max_connections: Option<u32>,
}

pub enum DiscoveryOut {
	Discovered { peer_id: PeerId },
}

pub struct Discovery {
	local_peer_id: PeerId,
	user_defined: Vec<(PeerId, Multiaddr)>,
	kademlia: Kademlia<MemoryStore>,
	random_query_timer: Delay,
	random_query_duration: Duration,
	cached_external_addresses: LruCache<Multiaddr, ()>,
	max_connections: Option<u32>,
	num_connections: u32,
}

impl Discovery {
	pub fn new(config: DiscoveryConfig) -> Self {
		let store = MemoryStore::new(config.local_peer_id.clone());

		let mut kademlia_config = KademliaConfig::default();
		kademlia_config.set_kbucket_inserts(KademliaBucketInserts::Manual);
		let mut kademlia =
			Kademlia::with_config(config.local_peer_id.clone(), store, kademlia_config);
		for (peer_id, addr) in &config.user_defined {
			kademlia.add_address(peer_id, addr.clone());
		}

		Self {
			local_peer_id: config.local_peer_id,
			user_defined: config.user_defined,
			kademlia,
			random_query_timer: delay_for(Duration::from_secs(0)),
			random_query_duration: Duration::from_secs(1),
			cached_external_addresses: LruCache::new(32),
			max_connections: config.max_connections,
			num_connections: 0,
		}
	}

	pub fn add_address(&mut self, peer: &PeerId, address: Multiaddr) {
		self.kademlia.add_address(peer, address);
	}
}

impl NetworkBehaviour for Discovery {
	type ProtocolsHandler = <Kademlia<MemoryStore> as NetworkBehaviour>::ProtocolsHandler;
	type OutEvent = DiscoveryOut;

	fn new_handler(&mut self) -> Self::ProtocolsHandler {
		NetworkBehaviour::new_handler(&mut self.kademlia)
	}

	fn addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
		let mut list = self
			.user_defined
			.iter()
			.filter_map(|(p, a)| if p == peer_id { Some(a.clone()) } else { None })
			.collect::<Vec<_>>();
		list.extend(self.kademlia.addresses_of_peer(peer_id));
		list
	}

	fn inject_connected(&mut self, peer_id: &PeerId) {
		NetworkBehaviour::inject_connected(&mut self.kademlia, peer_id);
	}

	fn inject_disconnected(&mut self, peer_id: &PeerId) {
		NetworkBehaviour::inject_disconnected(&mut self.kademlia, peer_id);
	}

	fn inject_connection_established(
		&mut self,
		peer_id: &PeerId,
		conn: &ConnectionId,
		endpoint: &ConnectedPoint,
	) {
		self.num_connections += 1;
		NetworkBehaviour::inject_connection_established(
			&mut self.kademlia,
			peer_id,
			conn,
			endpoint,
		);
	}

	fn inject_connection_closed(
		&mut self,
		peer_id: &PeerId,
		conn: &ConnectionId,
		endpoint: &ConnectedPoint,
	) {
		self.num_connections -= 1;
		NetworkBehaviour::inject_connection_closed(&mut self.kademlia, peer_id, conn, endpoint);
	}

	fn inject_event(
		&mut self,
		peer_id: PeerId,
		connection: ConnectionId,
		event: <<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::OutEvent,
	) {
		NetworkBehaviour::inject_event(&mut self.kademlia, peer_id, connection, event);
	}

	fn inject_addr_reach_failure(
		&mut self,
		peer_id: Option<&PeerId>,
		addr: &Multiaddr,
		error: &dyn std::error::Error,
	) {
		NetworkBehaviour::inject_addr_reach_failure(&mut self.kademlia, peer_id, addr, error);
	}

	fn inject_dial_failure(&mut self, peer_id: &PeerId) {
		NetworkBehaviour::inject_dial_failure(&mut self.kademlia, peer_id)
	}

	fn inject_new_listen_addr(&mut self, addr: &Multiaddr) {
		NetworkBehaviour::inject_new_listen_addr(&mut self.kademlia, addr)
	}

	fn inject_expired_listen_addr(&mut self, addr: &Multiaddr) {
		NetworkBehaviour::inject_expired_listen_addr(&mut self.kademlia, addr)
	}

	fn inject_new_external_addr(&mut self, addr: &Multiaddr) {
		let new_addr = addr
			.clone()
			.with(Protocol::P2p(self.local_peer_id.clone().into()));
		if self
			.cached_external_addresses
			.put(new_addr.clone(), ())
			.is_none()
		{
			info!(
				"Discovered new external address of current node: {}",
				new_addr
			);
		}
		NetworkBehaviour::inject_new_external_addr(&mut self.kademlia, addr)
	}

	fn inject_listener_error(&mut self, id: ListenerId, err: &(dyn std::error::Error + 'static)) {
		NetworkBehaviour::inject_listener_error(&mut self.kademlia, id, err)
	}

	fn inject_listener_closed(&mut self, id: ListenerId, reason: Result<(), &io::Error>) {
		NetworkBehaviour::inject_listener_closed(&mut self.kademlia, id, reason)
	}

	fn poll(&mut self, cx: &mut Context<'_>, params: &mut impl PollParameters) -> Poll<NetworkBehaviourAction<<<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::InEvent, Self::OutEvent>>{
		while let Poll::Ready(_) = self.random_query_timer.poll_unpin(cx) {
			if self.num_connections <= self.max_connections.unwrap_or(u32::MAX) {
				let random_peer_id = PeerId::random();
				debug!("Libp2p <= Random query({:?})", random_peer_id);
				self.kademlia.get_closest_peers(random_peer_id);

				self.random_query_timer = delay_for(self.random_query_duration);
				self.random_query_duration =
					cmp::min(self.random_query_duration * 2, Duration::from_secs(60));
			}
		}

		while let Poll::Ready(ev) = self.kademlia.poll(cx, params) {
			match ev {
				NetworkBehaviourAction::GenerateEvent(ev) => match ev {
					KademliaEvent::RoutingUpdated { peer, .. } => {
						let ev = DiscoveryOut::Discovered { peer_id: peer };
						return Poll::Ready(NetworkBehaviourAction::GenerateEvent(ev));
					}
					KademliaEvent::RoutablePeer { peer, .. } => {
						let ev = DiscoveryOut::Discovered { peer_id: peer };
						return Poll::Ready(NetworkBehaviourAction::GenerateEvent(ev));
					}
					ev => {
						trace!("Got Kademlia out event: {:?}", ev);
					}
				},
				NetworkBehaviourAction::DialAddress { address } => {
					return Poll::Ready(NetworkBehaviourAction::DialAddress { address });
				}
				NetworkBehaviourAction::DialPeer { peer_id, condition } => {
					return Poll::Ready(NetworkBehaviourAction::DialPeer { peer_id, condition });
				}
				NetworkBehaviourAction::NotifyHandler {
					peer_id,
					handler,
					event,
				} => {
					return Poll::Ready(NetworkBehaviourAction::NotifyHandler {
						peer_id,
						handler,
						event,
					});
				}
				NetworkBehaviourAction::ReportObservedAddr { address, score } => {
					return Poll::Ready(NetworkBehaviourAction::ReportObservedAddr {
						address,
						score,
					});
				}
			}
		}
		Poll::Pending
	}
}
