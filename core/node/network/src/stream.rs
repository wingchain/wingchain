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

use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::task::{Context, Poll};
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use libp2p::bandwidth::BandwidthSinks;
use libp2p::core::{ConnectedPoint, Multiaddr};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{PeerId, Swarm};
use log::{debug, error};

use crate::behaviour::{Behaviour, BehaviourOut};
use crate::{NetWorkOutMessage, NetworkInMessage};

pub async fn start(mut stream: NetworkStream) {
	loop {
		match stream.next().await {
			Some(_) => (),
			None => break,
		}
	}
}

pub struct NetworkStream {
	pub swarm: Swarm<Behaviour>,
	#[allow(dead_code)]
	pub bandwidth: Arc<BandwidthSinks>,
	pub in_rx: UnboundedReceiver<NetworkInMessage>,
	pub out_tx: UnboundedSender<NetWorkOutMessage>,
}

#[derive(Debug)]
pub struct NetworkState {
	pub peer_id: PeerId,
	pub listened_addresses: HashSet<Multiaddr>,
	pub external_addresses: HashSet<Multiaddr>,
	pub opened_peers: Vec<OpenedPeer>,
	pub unopened_peers: Vec<UnopenedPeer>,
}

#[derive(Debug)]
pub struct OpenedPeer {
	peer_id: PeerId,
	connected_point: ConnectedPoint,
	known_addresses: HashSet<Multiaddr>,
	agent_version: Option<String>,
	latest_ping: Option<Duration>,
}

#[derive(Debug)]
pub struct UnopenedPeer {
	peer_id: PeerId,
	known_addresses: HashSet<Multiaddr>,
}

impl NetworkStream {
	fn network_state(&mut self) -> NetworkState {
		let peer_id = Swarm::local_peer_id(&self.swarm).clone();
		let listened_addresses = Swarm::listeners(&self.swarm).cloned().collect();
		let external_addresses = Swarm::external_addresses(&self.swarm)
			.map(|x| x.addr.clone())
			.collect();
		let opened_peers = {
			let peers = self
				.swarm
				.peers()
				.iter()
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect::<Vec<_>>();
			let swarm = &mut self.swarm;
			peers
				.into_iter()
				.map(|(peer_id, peer_info)| {
					let known_addresses =
						NetworkBehaviour::addresses_of_peer(&mut **swarm, &peer_id)
							.into_iter()
							.collect();
					OpenedPeer {
						peer_id,
						connected_point: peer_info.connected_point,
						known_addresses,
						agent_version: peer_info.agent_version,
						latest_ping: peer_info.latest_ping,
					}
				})
				.collect::<Vec<_>>()
		};
		let unopened_peers = {
			let opened_peers_id_set = opened_peers
				.iter()
				.map(|x| &x.peer_id)
				.collect::<HashSet<_>>();
			self.swarm
				.known_peers()
				.into_iter()
				.filter(|peer_id| !opened_peers_id_set.contains(peer_id))
				.map(|peer_id| {
					let known_addresses =
						NetworkBehaviour::addresses_of_peer(&mut *self.swarm, &peer_id)
							.into_iter()
							.collect();
					UnopenedPeer {
						peer_id,
						known_addresses,
					}
				})
				.collect()
		};
		NetworkState {
			peer_id,
			listened_addresses,
			external_addresses,
			opened_peers,
			unopened_peers,
		}
	}
}

impl Stream for NetworkStream {
	type Item = ();

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		// in_rx
		loop {
			match self.in_rx.poll_next_unpin(cx) {
				Poll::Ready(Some(out)) => match out {
					NetworkInMessage::SendMessage { peer_id, message } => {
						self.swarm.send_message(peer_id, message);
					}
					NetworkInMessage::DropPeer { peer_id, delay } => {
						self.swarm.drop_peer(peer_id, delay);
					}
					NetworkInMessage::GetNetworkState { tx } => {
						let _ = tx.send(self.network_state());
					}
				},
				Poll::Ready(None) => break,
				Poll::Pending => break,
			}
		}

		// swarm
		let out_tx = self.out_tx.clone();
		loop {
			let next_event = self.swarm.next_event();
			futures::pin_mut!(next_event);
			match next_event.poll_unpin(cx) {
				Poll::Ready(event) => match event {
					SwarmEvent::Behaviour(behaviour_out) => {
						let network_out_message = behaviour_out.into();
						out_tx
							.unbounded_send(network_out_message)
							.unwrap_or_else(|e| error!("Network out message send error: {}", e));
					}
					_ => {
						debug!("Network event: {:?}", event);
					}
				},
				Poll::Pending => break,
			}
		}

		Poll::Pending
	}
}

impl From<BehaviourOut> for NetWorkOutMessage {
	fn from(v: BehaviourOut) -> Self {
		match v {
			BehaviourOut::ProtocolOpen {
				peer_id,
				connected_point,
				handshake,
			} => NetWorkOutMessage::ProtocolOpen {
				peer_id,
				connected_point,
				handshake,
			},
			BehaviourOut::ProtocolClose {
				peer_id,
				connected_point,
			} => NetWorkOutMessage::ProtocolClose {
				peer_id,
				connected_point,
			},
			BehaviourOut::Message { message } => NetWorkOutMessage::Message { message },
		}
	}
}
