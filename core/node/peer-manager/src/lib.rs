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

use std::collections::VecDeque;
use std::pin::Pin;

use futures::task::{Context, Poll};
use futures::Stream;
use libp2p::PeerId;
use linked_hash_map::LinkedHashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

mod errors;

pub struct PeerManagerConfig {
	pub max_in_peers: u32,
	pub max_out_peers: u32,
	pub bootnodes: LinkedHashMap<PeerId, ()>,
	pub reserved: LinkedHashMap<PeerId, ()>,
	pub reserved_only: bool,
}

#[derive(Clone, PartialEq)]
pub enum PeerState {
	In,
	Out,
}

#[derive(Clone, PartialEq)]
pub enum PeerType {
	Reserved,
	Normal,
}

#[derive(Clone, PartialEq)]
pub struct PeerInfo {
	peer_state: PeerState,
	peer_type: PeerType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IncomingId(pub u64);

#[derive(Debug, PartialEq)]
pub enum OutMessage {
	Connect(PeerId),
	Drop(PeerId),
	Accept(IncomingId),
	Reject(IncomingId),
}

pub enum InMessage {
	AddReservedPeer(PeerId),
	RemoveReservedPeer(PeerId),
	SetReservedOnly(bool),
}

pub struct PeerManager {
	/// config
	config: PeerManagerConfig,
	/// maintain active peers
	active: ActivePeers,
	/// maintain inactive peers
	inactive: InactivePeers,
	/// in messages sender
	in_tx: UnboundedSender<InMessage>,
	/// in message receiver
	in_rx: UnboundedReceiver<InMessage>,
	/// out messages
	out_messages: VecDeque<OutMessage>,
}

impl PeerManager {
	pub fn new(config: PeerManagerConfig) -> Self {
		let active = ActivePeers {
			peers: Default::default(),
			in_peers: 0,
			out_peers: 0,
		};

		let inactive_peers = config
			.reserved
			.clone()
			.into_iter()
			.chain(config.bootnodes.clone().into_iter())
			.collect();

		let inactive = InactivePeers {
			peers: inactive_peers,
		};

		let (in_tx, in_rx) = unbounded_channel();

		let mut peer_manager = Self {
			config,
			active,
			inactive,
			in_tx,
			in_rx,
			out_messages: VecDeque::new(),
		};
		peer_manager.activate();

		peer_manager
	}

	pub fn tx(&self) -> UnboundedSender<InMessage> {
		self.in_tx.clone()
	}

	pub fn discovered(&mut self, peer_id: PeerId) {
		if self.active.contains(&peer_id) {
			return;
		}
		self.inactive.insert_peer(peer_id);
		self.activate();
	}

	pub fn dropped(&mut self, peer_id: PeerId) {
		self.active.remove_peer(&peer_id);
		self.inactive.insert_peer(peer_id);
		self.activate();
	}

	pub fn incoming(&mut self, peer_id: PeerId, incoming_id: IncomingId) {
		if self.config.reserved_only {
			if !self.config.reserved.contains_key(&peer_id) {
				self.send(OutMessage::Reject(incoming_id));
				return;
			}
		}

		match self
			.active
			.insert_peer(peer_id, PeerState::In, &self.config)
		{
			InsertPeerResult::Inserted(peer_id) => {
				self.inactive.remove_peer(peer_id);
				self.send(OutMessage::Accept(incoming_id));
			}
			InsertPeerResult::Replaced { inserted, removed } => {
				self.inactive.remove_peer(inserted);
				self.inactive.insert_peer(removed.clone());
				self.send(OutMessage::Accept(incoming_id));
				self.send(OutMessage::Drop(removed));
			}
			InsertPeerResult::Exist(_peer_id) => {}
			InsertPeerResult::Full(peer_id) => {
				self.inactive.insert_peer(peer_id);
				self.send(OutMessage::Reject(incoming_id));
			}
		}
	}

	fn activate(&mut self) {
		while let Some(peer_id) = self.inactive.take_peer(&self.config) {
			match self
				.active
				.insert_peer(peer_id, PeerState::Out, &self.config)
			{
				InsertPeerResult::Inserted(peer_id) => {
					self.send(OutMessage::Connect(peer_id));
				}
				InsertPeerResult::Replaced { inserted, removed } => {
					self.inactive.insert_peer(removed.clone());
					self.send(OutMessage::Connect(inserted));
					self.send(OutMessage::Drop(removed));
				}
				InsertPeerResult::Exist(_peer_id) => {}
				InsertPeerResult::Full(peer_id) => {
					self.inactive.insert_peer(peer_id);
					break;
				}
			}
		}
	}

	fn send(&mut self, message: OutMessage) {
		self.out_messages.push_back(message);
	}

	fn on_add_reserved_peer(&mut self, peer_id: PeerId) {
		self.config.reserved.insert(peer_id.clone(), ());
		if self.active.contains(&peer_id) {
			return;
		}
		self.inactive.insert_peer(peer_id);
		self.activate();
	}

	fn on_remove_reserved_peer(&mut self, peer_id: PeerId) {
		self.config.reserved.remove(&peer_id);
		if self.config.reserved_only {
			self.active.remove_peer(&peer_id);
			self.inactive.insert_peer(peer_id.clone());
			self.send(OutMessage::Drop(peer_id));
		}
		self.activate();
	}

	fn on_set_reserved_only(&mut self, reserved_only: bool) {
		self.config.reserved_only = reserved_only;
		if self.config.reserved_only {
			let active_normal_peers = self
				.active
				.peers
				.iter()
				.filter_map(|(peer_id, _)| {
					if !self.config.reserved.contains_key(peer_id) {
						Some(peer_id.clone())
					} else {
						None
					}
				})
				.collect::<Vec<_>>();
			for peer_id in active_normal_peers {
				self.active.remove_peer(&peer_id);
				self.inactive.insert_peer(peer_id.clone());
				self.send(OutMessage::Drop(peer_id));
			}
		}
		self.activate();
	}
}

impl Stream for PeerManager {
	type Item = OutMessage;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		loop {
			if let Some(message) = self.out_messages.pop_front() {
				return Poll::Ready(Some(message));
			}

			let in_message = match Stream::poll_next(Pin::new(&mut self.in_rx), cx) {
				Poll::Pending => return Poll::Pending,
				Poll::Ready(Some(in_message)) => in_message,
				Poll::Ready(None) => return Poll::Pending,
			};

			match in_message {
				InMessage::AddReservedPeer(peer_id) => self.on_add_reserved_peer(peer_id),
				InMessage::RemoveReservedPeer(peer_id) => self.on_remove_reserved_peer(peer_id),
				InMessage::SetReservedOnly(reserved) => self.on_set_reserved_only(reserved),
			}
		}
	}
}

struct ActivePeers {
	peers: LinkedHashMap<PeerId, PeerInfo>,
	in_peers: u32,
	out_peers: u32,
}

impl ActivePeers {
	fn insert_peer(
		&mut self,
		peer_id: PeerId,
		peer_state: PeerState,
		config: &PeerManagerConfig,
	) -> InsertPeerResult {
		let peer_type = match config.reserved.contains_key(&peer_id) {
			true => PeerType::Reserved,
			false => PeerType::Normal,
		};

		if let Some(_peer_info) = self.peers.get(&peer_id) {
			return InsertPeerResult::Exist(peer_id);
		}
		let exceed_max_peers = match peer_state {
			PeerState::In => self.in_peers >= config.max_in_peers,
			PeerState::Out => self.out_peers >= config.max_out_peers,
		};
		if exceed_max_peers {
			if peer_type == PeerType::Reserved {
				let to_remove = self
					.peers
					.iter()
					.find(|peer| {
						peer.1.peer_state == peer_state && peer.1.peer_type == PeerType::Normal
					})
					.map(|(peer_id, peer_info)| (peer_id.clone(), peer_info.peer_state.clone()));

				if let Some((to_remove_peer_id, to_remove_peer_state)) = to_remove {
					// insert
					match peer_state {
						PeerState::In => self.in_peers += 1,
						PeerState::Out => self.out_peers += 1,
					};
					self.peers.insert(
						peer_id.clone(),
						PeerInfo {
							peer_type,
							peer_state,
						},
					);

					// remove
					match to_remove_peer_state {
						PeerState::In => self.in_peers -= 1,
						PeerState::Out => self.out_peers -= 1,
					};
					self.peers.remove(&to_remove_peer_id);

					return InsertPeerResult::Replaced {
						inserted: peer_id,
						removed: to_remove_peer_id,
					};
				}
			}
			return InsertPeerResult::Full(peer_id);
		}

		//insert
		match peer_state {
			PeerState::In => self.in_peers += 1,
			PeerState::Out => self.out_peers += 1,
		};
		self.peers.insert(
			peer_id.clone(),
			PeerInfo {
				peer_type,
				peer_state,
			},
		);
		InsertPeerResult::Inserted(peer_id)
	}

	fn remove_peer(&mut self, peer_id: &PeerId) {
		let to_remove = self
			.peers
			.get(peer_id)
			.map(|peer_info| peer_info.peer_state.clone());
		if let Some(to_remove_peer_state) = to_remove {
			match to_remove_peer_state {
				PeerState::In => self.in_peers -= 1,
				PeerState::Out => self.out_peers -= 1,
			};
			self.peers.remove(peer_id);
		}
	}

	fn contains(&self, peer_id: &PeerId) -> bool {
		self.peers.contains_key(peer_id)
	}
}

enum InsertPeerResult {
	Inserted(PeerId),
	Replaced { inserted: PeerId, removed: PeerId },
	Exist(PeerId),
	Full(PeerId),
}

struct InactivePeers {
	peers: LinkedHashMap<PeerId, ()>,
}

impl InactivePeers {
	fn take_peer(&mut self, config: &PeerManagerConfig) -> Option<PeerId> {
		// reserved first
		if let Some(peer_id) = self.peers.iter().find_map(|(peer_id, ())| {
			if config.reserved.contains_key(peer_id) {
				Some(peer_id.clone())
			} else {
				None
			}
		}) {
			self.peers.remove(&peer_id);
			return Some(peer_id);
		}
		if config.reserved_only {
			return None;
		}

		// normal second
		if let Some(peer_id) = self.peers.iter().find_map(|(peer_id, ())| {
			if !config.reserved.contains_key(peer_id) {
				Some(peer_id.clone())
			} else {
				None
			}
		}) {
			self.peers.remove(&peer_id);
			return Some(peer_id);
		}
		None
	}
	fn insert_peer(&mut self, peer_id: PeerId) {
		self.peers.insert(peer_id, ());
	}
	fn remove_peer(&mut self, peer_id: PeerId) {
		self.peers.remove(&peer_id);
	}
}
