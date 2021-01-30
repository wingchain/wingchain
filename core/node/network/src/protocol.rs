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

use rand::Rng;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::error;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use fnv::FnvHashMap;
use futures::FutureExt;
use futures::StreamExt;
use futures_codec::BytesMut;
use futures_timer::Delay;
use libp2p::core::connection::ConnectionId;
use libp2p::core::{ConnectedPoint, Multiaddr};
use libp2p::swarm::{
	DialPeerCondition, NetworkBehaviour, NetworkBehaviourAction, NotifyHandler, PollParameters,
};
use libp2p::PeerId;
use log::{debug, info};

use node_peer_manager::{IncomingId, OutMessage, PeerManager};

use crate::peer_report::{PEER_REPORT_DIAL_FAILURE, PEER_REPORT_PROTOCOL_ERROR};
use crate::protocol::handler::{HandlerIn, HandlerOut, HandlerProto};
use crate::{HandshakeBuilder, PMInMessage};
use rand::thread_rng;

mod handler;
mod upgrade;

const PROTOCOL_NAME: &[u8] = b"/wingchain/protocol/1.0.0";
const DELAY: Duration = Duration::from_secs(5);

pub enum ProtocolOut {
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

#[derive(Clone)]
pub struct Connection {
	connected_point: ConnectedPoint,
	incoming_id: Option<IncomingId>,
}

pub enum PeerState {
	Init {
		pending_dial: Option<Delay>,
	},
	Connected {
		/// we should maintain all the connections when at `Connected` state
		/// otherwise, if the outgoing connection comes before the incoming connection
		/// we cannot reach ProtocolOpened
		connected_list: FnvHashMap<ConnectionId, Connection>,
		nonce: u64,
	},
	ProtocolOpened {
		connected_list: FnvHashMap<ConnectionId, Connection>,
		opened_list: FnvHashMap<ConnectionId, Connection>,
		nonce: u64,
	},
	Locked,
}

pub struct ProtocolConfig {
	pub local_peer_id: PeerId,
	pub handshake_builder: Arc<dyn HandshakeBuilder>,
}

pub struct Protocol {
	local_peer_id: PeerId,
	handshake_builder: Arc<dyn HandshakeBuilder>,
	peers: FnvHashMap<PeerId, PeerState>,
	incoming_peers: FnvHashMap<IncomingId, PeerId>,
	delay_peers: FnvHashMap<PeerId, Instant>,
	peer_manager: PeerManager,
	next_incoming_id: IncomingId,
	events: VecDeque<NetworkBehaviourAction<HandlerIn, ProtocolOut>>,
}

impl Protocol {
	pub fn new(config: ProtocolConfig, peer_manager: PeerManager) -> Self {
		Self {
			local_peer_id: config.local_peer_id,
			handshake_builder: config.handshake_builder,
			peers: FnvHashMap::default(),
			incoming_peers: FnvHashMap::default(),
			delay_peers: FnvHashMap::default(),
			peer_manager,
			next_incoming_id: IncomingId(0),
			events: VecDeque::with_capacity(16),
		}
	}

	pub fn add_discovered_peers(&mut self, peer_ids: impl Iterator<Item = PeerId>) {
		for peer_id in peer_ids {
			if peer_id != self.local_peer_id {
				self.peer_manager.discovered(peer_id);
			}
		}
	}

	pub fn send_message(&mut self, peer_id: PeerId, message: Vec<u8>) {
		let opened_list = match self.peers.get(&peer_id) {
			Some(PeerState::ProtocolOpened { opened_list, .. }) => opened_list,
			_ => {
				info!("Send message to an unopened peer: {}", peer_id);
				return;
			}
		};
		let connection_id = opened_list.iter().next().map(|(k, _v)| k);
		if let Some(connection_id) = connection_id {
			self.events
				.push_back(NetworkBehaviourAction::NotifyHandler {
					peer_id,
					handler: NotifyHandler::One(*connection_id),
					event: HandlerIn::SendMessage { message },
				});
		}
	}

	pub fn drop_peer(&mut self, peer_id: PeerId, delay: Option<Duration>) {
		if let Some(delay) = delay {
			self.delay_peers
				.insert(peer_id.clone(), Instant::now() + delay);
		}
		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				*entry = PeerState::Init { pending_dial };
			}
			// Connected => Connected
			PeerState::Connected {
				connected_list,
				nonce,
			} => {
				debug!("External => Drop({})", peer_id);
				debug!("Handler({}, All) <= Close", peer_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id,
						handler: NotifyHandler::All,
						event: HandlerIn::Close,
					});
				*entry = PeerState::Connected {
					connected_list,
					nonce,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connected_list,
				opened_list,
				nonce,
			} => {
				debug!("External => Drop({})", peer_id);
				debug!("Handler({}, All) <= Close", peer_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id,
						handler: NotifyHandler::All,
						event: HandlerIn::Close,
					});
				*entry = PeerState::ProtocolOpened {
					connected_list,
					opened_list,
					nonce,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn next_incoming_id(incoming_id: &mut IncomingId) -> IncomingId {
		let new = IncomingId(incoming_id.0.checked_add(1).unwrap_or(0));
		std::mem::replace(incoming_id, new)
	}

	fn peer_manager_connect(&mut self, peer_id: PeerId) {
		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				debug!("PeerManager => Connect({})", peer_id);
				if peer_id == self.local_peer_id {
					// condition: connect to self
					debug!("Ignore connecting to self: {}", peer_id);
					*entry = PeerState::Init { pending_dial };
				} else {
					// condition: connect to other
					match self.delay_peers.remove(&peer_id) {
						Some(instant) if instant > Instant::now() => {
							// condition: need schedule pending
							debug!("Schedule pending dial({}) at {:?}", peer_id, instant);
							*entry = PeerState::Init {
								pending_dial: Some(Delay::new(instant - Instant::now())),
							};
						}
						_ => match pending_dial {
							// condition: not need schedule pending
							None => {
								// condition: connect immediately
								debug!("Libp2p <= Dial({})", peer_id);
								self.events.push_back(NetworkBehaviourAction::DialPeer {
									peer_id: peer_id.clone(),
									condition: DialPeerCondition::Disconnected,
								});
								*entry = PeerState::Init { pending_dial };
							}
							Some(_) => {
								// condition: connect later
								debug!("Pending dial({})", peer_id);
								*entry = PeerState::Init { pending_dial };
							}
						},
					}
				}
			}
			// Connected => Connected
			PeerState::Connected {
				connected_list,
				nonce,
			} => {
				*entry = PeerState::Connected {
					connected_list,
					nonce,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connected_list,
				opened_list,
				nonce,
			} => {
				*entry = PeerState::ProtocolOpened {
					connected_list,
					opened_list,
					nonce,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn peer_manager_drop(&mut self, peer_id: PeerId) {
		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				*entry = PeerState::Init { pending_dial };
			}
			// Connected => Connected
			PeerState::Connected {
				connected_list,
				nonce,
			} => {
				debug!("PeerManager => Drop({})", peer_id);
				debug!("Handler({}, All) <= Close", peer_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id,
						handler: NotifyHandler::All,
						event: HandlerIn::Close,
					});
				*entry = PeerState::Connected {
					connected_list,
					nonce,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connected_list,
				opened_list,
				nonce,
			} => {
				debug!("PeerManager => Drop({})", peer_id);
				debug!("Handler({}, All) <= Close", peer_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id,
						handler: NotifyHandler::All,
						event: HandlerIn::Close,
					});
				*entry = PeerState::ProtocolOpened {
					connected_list,
					opened_list,
					nonce,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn peer_manager_accept(&mut self, incoming_id: IncomingId) {
		let peer_id = match self.incoming_peers.remove(&incoming_id) {
			Some(peer_id) => peer_id,
			None => return,
		};

		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });

		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				debug!("PeerManager => Accept({})", peer_id);
				debug!("PeerManager <= Dropped({})", peer_id);
				self.peer_manager.dropped(peer_id);
				*entry = PeerState::Init { pending_dial };
			}
			// Connected => Connected
			PeerState::Connected {
				connected_list,
				nonce,
			} => {
				let connection_id = connected_list.iter().find_map(|(k, v)| {
					if v.incoming_id.as_ref() == Some(&incoming_id) {
						Some(k)
					} else {
						None
					}
				});
				if let Some(connection_id) = connection_id {
					debug!("PeerManager => Accept({})", peer_id);
					debug!("Handler({}, {:?}) <= Open", peer_id, connection_id);
					let handshake = self.handshake_builder.build(nonce);
					self.events
						.push_back(NetworkBehaviourAction::NotifyHandler {
							peer_id,
							handler: NotifyHandler::One(*connection_id),
							event: HandlerIn::Open { handshake },
						});
				}
				*entry = PeerState::Connected {
					connected_list,
					nonce,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connected_list,
				opened_list,
				nonce,
			} => {
				let connection_id = connected_list.iter().find_map(|(k, v)| {
					if v.incoming_id.as_ref() == Some(&incoming_id) {
						Some(k)
					} else {
						None
					}
				});
				if let Some(connection_id) = connection_id {
					debug!("PeerManager => Accept({})", peer_id);
					debug!("Handler({}, {:?}) <= Open", peer_id, connection_id);
					let handshake = self.handshake_builder.build(nonce);
					self.events
						.push_back(NetworkBehaviourAction::NotifyHandler {
							peer_id,
							handler: NotifyHandler::One(*connection_id),
							event: HandlerIn::Open { handshake },
						});
				}
				*entry = PeerState::ProtocolOpened {
					connected_list,
					opened_list,
					nonce,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn peer_manager_reject(&mut self, incoming_id: IncomingId) {
		let peer_id = match self.incoming_peers.remove(&incoming_id) {
			Some(peer_id) => peer_id,
			None => return,
		};

		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });

		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				*entry = PeerState::Init { pending_dial };
			}
			// Connected => Connected
			PeerState::Connected {
				connected_list,
				nonce,
			} => {
				debug!("PeerManager => Reject({})", peer_id);
				debug!("Handler({}, All) <= Close", peer_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id,
						handler: NotifyHandler::All,
						event: HandlerIn::Close,
					});
				*entry = PeerState::Connected {
					connected_list,
					nonce,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connected_list,
				opened_list,
				nonce,
			} => {
				debug!("PeerManager => Reject({})", peer_id);
				debug!("Handler({}, All) <= Close", peer_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id,
						handler: NotifyHandler::All,
						event: HandlerIn::Close,
					});
				*entry = PeerState::ProtocolOpened {
					connected_list,
					opened_list,
					nonce,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}
}

impl NetworkBehaviour for Protocol {
	type ProtocolsHandler = HandlerProto;
	type OutEvent = ProtocolOut;

	fn new_handler(&mut self) -> Self::ProtocolsHandler {
		HandlerProto::new(self.local_peer_id.clone(), Cow::Borrowed(PROTOCOL_NAME))
	}

	fn addresses_of_peer(&mut self, _: &PeerId) -> Vec<Multiaddr> {
		Vec::new()
	}

	fn inject_connected(&mut self, _: &PeerId) {}

	fn inject_disconnected(&mut self, _peer_id: &PeerId) {}

	fn inject_connection_established(
		&mut self,
		peer_id: &PeerId,
		conn: &ConnectionId,
		endpoint: &ConnectedPoint,
	) {
		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Connected
			PeerState::Init { .. } => {
				let mut connection = Connection {
					connected_point: endpoint.clone(),
					incoming_id: None,
				};
				let mut rng = thread_rng();
				let nonce = rng.gen();
				match endpoint {
					ConnectedPoint::Dialer { .. } => {
						// open in handler
						debug!("Libp2p => Connected({}): Through: {:?}", peer_id, endpoint);
						debug!("Handler({}, {:?}) <= Open", peer_id, conn);
						let handshake = self.handshake_builder.build(nonce);
						self.events
							.push_back(NetworkBehaviourAction::NotifyHandler {
								peer_id: peer_id.clone(),
								handler: NotifyHandler::One(*conn),
								event: HandlerIn::Open { handshake },
							});
					}
					ConnectedPoint::Listener { .. } => {
						match self.delay_peers.remove(peer_id) {
							Some(instant) if instant > Instant::now() => {
								// close in handler
								debug!("Libp2p => Connected({}): Through: {:?}", peer_id, endpoint);
								debug!("Handler({}, {:?}) <= Close", peer_id, conn);
								self.events
									.push_back(NetworkBehaviourAction::NotifyHandler {
										peer_id: peer_id.clone(),
										handler: NotifyHandler::One(*conn),
										event: HandlerIn::Close,
									});
								self.delay_peers.insert(peer_id.clone(), instant);
							}
							_ => {
								// determine by peer manager
								let incoming_id =
									Self::next_incoming_id(&mut self.next_incoming_id);
								debug!("Libp2p => Connected({}): Through: {:?}", peer_id, endpoint);
								debug!(
									"PeerManager <= Incoming({}, {:?}): Through: {:?}",
									peer_id, incoming_id, endpoint
								);
								self.incoming_peers
									.insert(incoming_id.clone(), peer_id.clone());
								self.peer_manager
									.incoming(peer_id.clone(), incoming_id.clone());
								connection.incoming_id = Some(incoming_id)
							}
						}
					}
				}
				debug!(
					"StateChange: Init -> Connected: local: {}, remote: {}",
					self.local_peer_id, peer_id
				);
				*entry = PeerState::Connected {
					connected_list: std::iter::once((*conn, connection)).collect(),
					nonce,
				}
			}
			// Connected => Connected
			PeerState::Connected {
				mut connected_list,
				nonce,
			} => {
				let should_open = match endpoint {
					ConnectedPoint::Dialer { .. } => true,
					ConnectedPoint::Listener { .. } => {
						let dialer_connection_exist = connected_list.iter().any(
							|(_k, v)| matches!(v.connected_point, ConnectedPoint::Dialer { .. }),
						);
						dialer_connection_exist
					}
				};

				if should_open {
					// open in handler
					debug!("Libp2p => Connected({}): Through: {:?}", peer_id, endpoint);
					debug!("Handler({}, {:?}) <= Open", peer_id, conn);
					let handshake = self.handshake_builder.build(nonce);
					self.events
						.push_back(NetworkBehaviourAction::NotifyHandler {
							peer_id: peer_id.clone(),
							handler: NotifyHandler::One(*conn),
							event: HandlerIn::Open { handshake },
						});
					connected_list.insert(
						*conn,
						Connection {
							connected_point: endpoint.clone(),
							incoming_id: None,
						},
					);
					debug!(
						"StateChange: Connected -> Connected ({}): local: {}, remote: {}",
						connected_list.len(),
						self.local_peer_id,
						peer_id
					);
				} else {
					self.events
						.push_back(NetworkBehaviourAction::NotifyHandler {
							peer_id: peer_id.clone(),
							handler: NotifyHandler::One(*conn),
							event: HandlerIn::Close,
						});
				}
				*entry = PeerState::Connected {
					connected_list,
					nonce,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				mut connected_list,
				opened_list,
				nonce,
			} => {
				let should_open = match endpoint {
					ConnectedPoint::Dialer { .. } => true,
					ConnectedPoint::Listener { .. } => {
						let dialer_connection_exist = connected_list.iter().any(
							|(_k, v)| matches!(v.connected_point, ConnectedPoint::Dialer { .. }),
						);
						dialer_connection_exist
					}
				};

				if should_open {
					// open in handler
					debug!("Libp2p => Connected({}): Through: {:?}", peer_id, endpoint);
					debug!("Handler({}, {:?}) <= Open", peer_id, conn);
					let handshake = self.handshake_builder.build(nonce);
					self.events
						.push_back(NetworkBehaviourAction::NotifyHandler {
							peer_id: peer_id.clone(),
							handler: NotifyHandler::One(*conn),
							event: HandlerIn::Open { handshake },
						});
					connected_list.insert(
						*conn,
						Connection {
							connected_point: endpoint.clone(),
							incoming_id: None,
						},
					);
				} else {
					self.events
						.push_back(NetworkBehaviourAction::NotifyHandler {
							peer_id: peer_id.clone(),
							handler: NotifyHandler::One(*conn),
							event: HandlerIn::Close,
						});
				}
				*entry = PeerState::ProtocolOpened {
					connected_list,
					opened_list,
					nonce,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	#[allow(clippy::unnecessary_unwrap)]
	fn inject_connection_closed(
		&mut self,
		peer_id: &PeerId,
		conn: &ConnectionId,
		_endpoint: &ConnectedPoint,
	) {
		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				*entry = PeerState::Init { pending_dial };
			}
			// Connected => Init
			PeerState::Connected {
				mut connected_list,
				nonce,
			} => {
				connected_list.remove(conn);
				if connected_list.is_empty() {
					debug!("Libp2p => Disconnected({})", peer_id);
					debug!("PeerManager <= Dropped({})", peer_id);
					self.peer_manager.dropped(peer_id.clone());
					debug!(
						"StateChange: Connected -> Init: local: {}, remote: {}",
						self.local_peer_id, peer_id
					);
					*entry = PeerState::Init { pending_dial: None };
				} else {
					*entry = PeerState::Connected {
						connected_list,
						nonce,
					};
				}
			}
			// ProtocolOpened => Init
			PeerState::ProtocolOpened {
				mut connected_list,
				mut opened_list,
				nonce,
			} => {
				let mut closed_connection = None;
				if let Some(connection) = connected_list.remove(conn) {
					closed_connection = Some(connection);
				}
				if let Some(connection) = opened_list.remove(conn) {
					closed_connection = Some(connection);
				}
				if closed_connection.is_some() && opened_list.is_empty() {
					let closed_connection = closed_connection.unwrap();
					debug!(
						"Libp2p => Disconnected({}): Through: {:?}",
						peer_id, closed_connection.connected_point
					);
					debug!(
						"External <= ProtocolClose({}): Through: {:?}",
						peer_id, closed_connection.connected_point
					);
					self.events.push_back(NetworkBehaviourAction::GenerateEvent(
						ProtocolOut::ProtocolClose {
							peer_id: peer_id.clone(),
							connected_point: closed_connection.connected_point,
						},
					));

					if connected_list.is_empty() {
						self.peer_manager.dropped(peer_id.clone());
						debug!(
							"StateChange: ProtocolOpened -> Init: local: {}, remote: {}",
							self.local_peer_id, peer_id
						);
						*entry = PeerState::Init { pending_dial: None };
					} else {
						debug!(
							"StateChange: ProtocolOpened -> Connected: local: {}, remote: {}",
							self.local_peer_id, peer_id
						);
						*entry = PeerState::Connected {
							connected_list,
							nonce,
						};
					}
				} else {
					*entry = PeerState::ProtocolOpened {
						connected_list,
						opened_list,
						nonce,
					};
				}
			}
			PeerState::Locked => unreachable!(),
		}
	}

	#[allow(clippy::unnecessary_unwrap)]
	fn inject_event(&mut self, source: PeerId, conn: ConnectionId, event: HandlerOut) {
		match event {
			HandlerOut::ProtocolOpen { handshake } => {
				let entry = self
					.peers
					.entry(source.clone())
					.or_insert(PeerState::Init { pending_dial: None });
				match std::mem::replace(entry, PeerState::Locked) {
					// Init => Init
					PeerState::Init { pending_dial } => {
						*entry = PeerState::Init { pending_dial };
					}
					// Connected => ProtocolOpened
					PeerState::Connected {
						mut connected_list,
						nonce,
					} => {
						if let Some(connection) = connected_list.remove(&conn) {
							debug!("Handler({}, {:?}) => ProtocolOpen", source, conn);
							debug!(
								"External <= ProtocolOpen({}): Through: {:?}",
								source, connection.connected_point
							);
							debug!(
								"StateChange: Connected -> ProtocolOpened: local: {}, remote: {}",
								self.local_peer_id, source
							);
							self.events.push_back(NetworkBehaviourAction::GenerateEvent(
								ProtocolOut::ProtocolOpen {
									peer_id: source,
									connected_point: connection.connected_point.clone(),
									nonce,
									handshake,
								},
							));
							*entry = PeerState::ProtocolOpened {
								connected_list,
								opened_list: std::iter::once((conn, connection)).collect(),
								nonce,
							};
						} else {
							*entry = PeerState::Connected {
								connected_list,
								nonce,
							};
						}
					}
					// ProtocolOpened => ProtocolOpened
					PeerState::ProtocolOpened {
						mut connected_list,
						mut opened_list,
						nonce,
					} => {
						if let Some(connection) = connected_list.remove(&conn) {
							debug!("Handler({}, {:?}) => ProtocolOpen", source, conn);
							debug!(
								"External <= ProtocolOpen({}): Through: {:?}",
								source, connection.connected_point
							);
							opened_list.insert(conn, connection);
							debug!("StateChange: ProtocolOpened -> ProtocolOpened ({}): local: {}, remote: {}", opened_list.len(), self.local_peer_id, source);
						}
						*entry = PeerState::ProtocolOpened {
							connected_list,
							opened_list,
							nonce,
						};
					}
					PeerState::Locked => unreachable!(),
				}
			}
			HandlerOut::ProtocolClose { reason } => {
				let entry = self
					.peers
					.entry(source.clone())
					.or_insert(PeerState::Init { pending_dial: None });
				match std::mem::replace(entry, PeerState::Locked) {
					// Init => Init
					PeerState::Init { pending_dial } => {
						*entry = PeerState::Init { pending_dial };
					}
					// Connected => Connected
					PeerState::Connected {
						connected_list,
						nonce,
					} => {
						*entry = PeerState::Connected {
							connected_list,
							nonce,
						};
					}
					// ProtocolOpened => Connected
					PeerState::ProtocolOpened {
						mut connected_list,
						mut opened_list,
						nonce,
					} => {
						// move from opened to connected
						let mut closed_connection = None;
						if let Some(connection) = opened_list.remove(&conn) {
							closed_connection = Some(connection);
						}
						if closed_connection.is_some() && opened_list.is_empty() {
							let closed_connection = closed_connection.unwrap();
							debug!("Handler({}, {:?}) => ProtocolClose", source, conn);
							debug!(
								"External <= ProtocolClose({}): Through: {:?} Reason: {}",
								source, closed_connection.connected_point, reason,
							);
							debug!(
								"StateChange: ProtocolOpened -> Connected: local: {}, remote: {}",
								self.local_peer_id, source
							);

							self.events.push_back(NetworkBehaviourAction::GenerateEvent(
								ProtocolOut::ProtocolClose {
									peer_id: source,
									connected_point: closed_connection.connected_point.clone(),
								},
							));
							connected_list.insert(conn, closed_connection);
							*entry = PeerState::Connected {
								connected_list,
								nonce,
							};
						} else {
							*entry = PeerState::ProtocolOpened {
								connected_list,
								opened_list,
								nonce,
							};
						}
					}
					PeerState::Locked => unreachable!(),
				}
			}
			HandlerOut::ProtocolError {
				should_disconnect,
				error,
			} => {
				let entry = self
					.peers
					.entry(source.clone())
					.or_insert(PeerState::Init { pending_dial: None });
				match std::mem::replace(entry, PeerState::Locked) {
					// Init => Init
					PeerState::Init { pending_dial } => {
						*entry = PeerState::Init { pending_dial };
					}
					// Connected => Connected
					PeerState::Connected {
						connected_list,
						nonce,
					} => {
						if let Some(_connection) = connected_list.get(&conn) {
							debug!(
								"Handler({}, {:?}) => ProtocolError({}, {})",
								source, conn, should_disconnect, error
							);
							if should_disconnect {
								debug!("Handler({}, {:?}) <= Close", source, conn);
								self.delay_peers
									.insert(source.clone(), Instant::now() + DELAY);
								self.events
									.push_back(NetworkBehaviourAction::NotifyHandler {
										peer_id: source.clone(),
										handler: NotifyHandler::One(conn),
										event: HandlerIn::Close,
									});
								let _ =
									self.peer_manager
										.tx()
										.unbounded_send(PMInMessage::ReportPeer(
											source,
											PEER_REPORT_PROTOCOL_ERROR,
										));
							}
						}
						*entry = PeerState::Connected {
							connected_list,
							nonce,
						};
					}
					// ProtocolOpened => ProtocolOpened
					PeerState::ProtocolOpened {
						connected_list,
						opened_list,
						nonce,
					} => {
						if connected_list.contains_key(&conn) || opened_list.contains_key(&conn) {
							debug!(
								"Handler({}, {:?}) => ProtocolError({}, {})",
								source, conn, should_disconnect, error
							);
							if should_disconnect {
								debug!("Handler({}, {:?}) <= Close", source, conn);
								self.delay_peers
									.insert(source.clone(), Instant::now() + DELAY);
								self.events
									.push_back(NetworkBehaviourAction::NotifyHandler {
										peer_id: source.clone(),
										handler: NotifyHandler::One(conn),
										event: HandlerIn::Close,
									});
								let _ =
									self.peer_manager
										.tx()
										.unbounded_send(PMInMessage::ReportPeer(
											source,
											PEER_REPORT_PROTOCOL_ERROR,
										));
							}
						}
						*entry = PeerState::ProtocolOpened {
							connected_list,
							opened_list,
							nonce,
						};
					}
					PeerState::Locked => unreachable!(),
				}
			}
			HandlerOut::Message { message } => {
				debug!("Handler({}, {:?}) => Message", source, conn);
				debug!("External <= Message({})", source);
				self.events.push_back(NetworkBehaviourAction::GenerateEvent(
					ProtocolOut::Message {
						peer_id: source,
						message,
					},
				));
			}
		}
	}

	fn inject_addr_reach_failure(
		&mut self,
		peer_id: Option<&PeerId>,
		addr: &Multiaddr,
		error: &dyn error::Error,
	) {
		debug!(
			"Libp2p => Reach failure for {:?} through {:?}: {:?}",
			peer_id, addr, error
		);
	}

	fn inject_dial_failure(&mut self, peer_id: &PeerId) {
		let entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { .. } => {
				debug!("Libp2p => Dial failure for {}", peer_id);
				debug!("PeerManager <= Dropped({})", peer_id);
				let _ = self
					.peer_manager
					.tx()
					.unbounded_send(PMInMessage::ReportPeer(
						peer_id.clone(),
						PEER_REPORT_DIAL_FAILURE,
					));
				self.peer_manager.dropped(peer_id.clone());
				self.delay_peers
					.insert(peer_id.clone(), Instant::now() + DELAY);
				*entry = PeerState::Init { pending_dial: None };
			}
			// Connected => Connected
			PeerState::Connected {
				connected_list,
				nonce,
			} => {
				*entry = PeerState::Connected {
					connected_list,
					nonce,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connected_list,
				opened_list,
				nonce,
			} => {
				*entry = PeerState::ProtocolOpened {
					connected_list,
					opened_list,
					nonce,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn poll(
		&mut self,
		cx: &mut Context,
		_params: &mut impl PollParameters,
	) -> Poll<NetworkBehaviourAction<HandlerIn, Self::OutEvent>> {
		// events
		if let Some(event) = self.events.pop_front() {
			return Poll::Ready(event);
		}

		// peer manager
		loop {
			match self.peer_manager.poll_next_unpin(cx) {
				Poll::Ready(Some(out)) => match out {
					OutMessage::Connect(peer_id) => {
						self.peer_manager_connect(peer_id);
					}
					OutMessage::Drop(peer_id) => {
						self.peer_manager_drop(peer_id);
					}
					OutMessage::Accept(incoming_id) => {
						self.peer_manager_accept(incoming_id);
					}
					OutMessage::Reject(incoming_id) => {
						self.peer_manager_reject(incoming_id);
					}
				},
				Poll::Ready(None) => break,
				Poll::Pending => break,
			}
		}

		// pending dial
		for (peer_id, peer_state) in self.peers.iter_mut() {
			if let PeerState::Init { pending_dial } = peer_state {
				if let Some(pd) = pending_dial {
					match pd.poll_unpin(cx) {
						Poll::Ready(_) => {
							// dial
							debug!("Apply pending dial({})", peer_id);
							debug!("Libp2p <= Dial({})", peer_id);
							self.events.push_back(NetworkBehaviourAction::DialPeer {
								peer_id: peer_id.clone(),
								condition: DialPeerCondition::Disconnected,
							});
							*pending_dial = None;
						}
						Poll::Pending => (),
					}
				}
			}
		}

		if let Some(event) = self.events.pop_front() {
			return Poll::Ready(event);
		}

		Poll::Pending
	}
}
