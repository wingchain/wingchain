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

use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::VecDeque;
use std::error;
use std::task::{Context, Poll};

use fnv::FnvHashMap;
use futures::FutureExt;
use futures::StreamExt;
use futures_codec::BytesMut;
use libp2p::core::connection::ConnectionId;
use libp2p::core::{ConnectedPoint, Multiaddr};
use libp2p::swarm::{
	DialPeerCondition, NetworkBehaviour, NetworkBehaviourAction, NotifyHandler, PollParameters,
};
use libp2p::PeerId;
use log::debug;
use tokio::time::{delay_until, Delay, Duration, Instant};

use node_peer_manager::{IncomingId, OutMessage, PeerManager};

use crate::protocol::handler::{Handler, HandlerIn, HandlerOut, HandlerProto};

mod handler;
mod upgrade;

const PROTOCOL_NAME: &'static [u8] = b"/wingchain/protocol/1.0.0";
const DELAY: Duration = Duration::from_secs(5);

pub enum ProtocolOut {
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

pub enum PeerState {
	Init {
		pending_dial: Option<Delay>,
	},
	Connected {
		connection_id: ConnectionId,
		connected_point: ConnectedPoint,
	},
	ProtocolOpened {
		connection_id: ConnectionId,
		connected_point: ConnectedPoint,
	},
	Locked,
}

pub struct ProtocolConfig {
	pub local_peer_id: PeerId,
}

pub struct Protocol {
	local_peer_id: PeerId,
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

	fn next_incoming_id(incoming_id: &mut IncomingId) -> IncomingId {
		let new = IncomingId(match incoming_id.0.checked_add(1) {
			Some(v) => v,
			None => 0,
		});
		std::mem::replace(incoming_id, new)
	}

	fn peer_manager_connect(&mut self, peer_id: PeerId) {
		let mut entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				debug!("PeerManager => Connect({})", peer_id);
				match self.delay_peers.remove(&peer_id) {
					Some(instant) if instant > Instant::now() => {
						debug!("Schedule pending dial({}) at {:?}", peer_id, instant);
						*entry = PeerState::Init {
							pending_dial: Some(delay_until(instant)),
						};
					}
					_ => match pending_dial {
						None => {
							debug!("Libp2p <= Dial({})", peer_id);
							self.events.push_back(NetworkBehaviourAction::DialPeer {
								peer_id: peer_id.clone(),
								condition: DialPeerCondition::Disconnected,
							});
							*entry = PeerState::Init { pending_dial };
						}
						Some(_) => {
							debug!("Pending dial({})", peer_id);
							*entry = PeerState::Init { pending_dial };
						}
					},
				}
			}
			// Connected => Connected
			PeerState::Connected {
				connection_id,
				connected_point,
			} => {
				*entry = PeerState::Connected {
					connection_id,
					connected_point,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connection_id,
				connected_point,
			} => {
				*entry = PeerState::ProtocolOpened {
					connection_id,
					connected_point,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn peer_manager_drop(&mut self, peer_id: PeerId) {
		let mut entry = self
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
				connection_id,
				connected_point,
			} => {
				debug!("PeerManager => Drop({})", peer_id);
				debug!("Handler({}, {:?}) <= Close", peer_id, connection_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id: peer_id.clone(),
						handler: NotifyHandler::One(connection_id),
						event: HandlerIn::Close,
					});
				*entry = PeerState::Connected {
					connection_id,
					connected_point,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connection_id,
				connected_point,
			} => {
				debug!("PeerManager => Drop({})", peer_id);
				debug!("Handler({}, {:?}) <= Close", peer_id, connection_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id: peer_id.clone(),
						handler: NotifyHandler::One(connection_id),
						event: HandlerIn::Close,
					});
				*entry = PeerState::ProtocolOpened {
					connection_id,
					connected_point,
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

		let mut entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });

		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				debug!("PeerManager => Accept({})", peer_id);
				debug!("PeerManager <= Dropped({})", peer_id);
				self.peer_manager.dropped(peer_id.clone());
				*entry = PeerState::Init { pending_dial };
			}
			// Connected => Connected
			PeerState::Connected {
				connection_id,
				connected_point,
			} => {
				debug!("PeerManager => Accept({})", peer_id);
				debug!("Handler({}, {:?}) <= Open", peer_id, connection_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id: peer_id.clone(),
						handler: NotifyHandler::One(connection_id),
						event: HandlerIn::Open,
					});
				*entry = PeerState::Connected {
					connection_id,
					connected_point,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connection_id,
				connected_point,
			} => {
				*entry = PeerState::ProtocolOpened {
					connection_id,
					connected_point,
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

		let mut entry = self
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
				connection_id,
				connected_point,
			} => {
				debug!("PeerManager => Reject({})", peer_id);
				debug!("Handler({}, {:?}) <= Close", peer_id, connection_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id: peer_id.clone(),
						handler: NotifyHandler::One(connection_id),
						event: HandlerIn::Close,
					});
				*entry = PeerState::Connected {
					connection_id,
					connected_point,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connection_id,
				connected_point,
			} => {
				debug!("PeerManager => Reject({})", peer_id);
				debug!("Handler({}, {:?}) <= Close", peer_id, connection_id);
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id: peer_id.clone(),
						handler: NotifyHandler::One(connection_id),
						event: HandlerIn::Close,
					});
				*entry = PeerState::ProtocolOpened {
					connection_id,
					connected_point,
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
		HandlerProto::new(Cow::Borrowed(PROTOCOL_NAME))
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
		let mut entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Connected
			PeerState::Init { pending_dial } => {
				match endpoint {
					ConnectedPoint::Dialer { .. } => {
						// open in handler
						debug!("Libp2p => Connected({}): Through: {:?}", peer_id, endpoint);
						debug!("Handler({}, {:?}) <= Open", peer_id, conn);
						self.events
							.push_back(NetworkBehaviourAction::NotifyHandler {
								peer_id: peer_id.clone(),
								handler: NotifyHandler::One(conn.clone()),
								event: HandlerIn::Open,
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
										handler: NotifyHandler::One(conn.clone()),
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
								self.peer_manager.incoming(peer_id.clone(), incoming_id);
							}
						}
					}
				}
				*entry = PeerState::Connected {
					connection_id: conn.clone(),
					connected_point: endpoint.clone(),
				}
			}
			// Connected => Connected
			PeerState::Connected {
				connection_id,
				connected_point,
			} => {
				// keep only 1 connection
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id: peer_id.clone(),
						handler: NotifyHandler::One(conn.clone()),
						event: HandlerIn::Close,
					});
				*entry = PeerState::Connected {
					connection_id,
					connected_point,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connection_id,
				connected_point,
			} => {
				// keep only 1 connection
				self.events
					.push_back(NetworkBehaviourAction::NotifyHandler {
						peer_id: peer_id.clone(),
						handler: NotifyHandler::One(conn.clone()),
						event: HandlerIn::Close,
					});
				*entry = PeerState::ProtocolOpened {
					connection_id,
					connected_point,
				};
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn inject_connection_closed(
		&mut self,
		peer_id: &PeerId,
		conn: &ConnectionId,
		endpoint: &ConnectedPoint,
	) {
		let mut entry = self
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
				connection_id,
				connected_point,
			} => {
				if &connection_id == conn {
					debug!(
						"Libp2p => Disconnected({}): Through: {:?}",
						peer_id, connected_point
					);
					debug!(
						"PeerManager <= Dropped({}): Through: {:?}",
						peer_id, connected_point
					);
					self.peer_manager.dropped(peer_id.clone());
					*entry = PeerState::Init { pending_dial: None };
				} else {
					*entry = PeerState::Connected {
						connection_id,
						connected_point,
					};
				}
			}
			// ProtocolOpened => Init
			PeerState::ProtocolOpened {
				connection_id,
				connected_point,
			} => {
				if &connection_id == conn {
					debug!(
						"Libp2p => Disconnected({}): Through: {:?}",
						peer_id, connected_point
					);
					debug!(
						"External <= ProtocolClose({}): Through: {:?}",
						peer_id, connected_point
					);
					debug!(
						"PeerManager <= Dropped({}): Through: {:?}",
						peer_id, connected_point
					);
					self.events.push_back(NetworkBehaviourAction::GenerateEvent(
						ProtocolOut::ProtocolClose {
							peer_id: peer_id.clone(),
							connected_point,
						},
					));
					self.peer_manager.dropped(peer_id.clone());
					*entry = PeerState::Init { pending_dial: None };
				} else {
					*entry = PeerState::ProtocolOpened {
						connection_id,
						connected_point,
					};
				}
			}
			PeerState::Locked => unreachable!(),
		}
	}

	fn inject_event(&mut self, source: PeerId, conn: ConnectionId, event: HandlerOut) {
		match event {
			HandlerOut::ProtocolOpen => {
				let mut entry = self
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
						connection_id,
						connected_point,
					} => {
						if conn == connection_id {
							debug!("Handler({}, {:?}) => ProtocolOpen", source, conn);
							debug!(
								"External <= ProtocolOpen({}): Through: {:?}",
								source, connected_point
							);
							self.events.push_back(NetworkBehaviourAction::GenerateEvent(
								ProtocolOut::ProtocolOpen {
									peer_id: source,
									connected_point: connected_point.clone(),
								},
							));
							*entry = PeerState::ProtocolOpened {
								connection_id,
								connected_point,
							};
						} else {
							*entry = PeerState::Connected {
								connection_id,
								connected_point,
							};
						}
					}
					// ProtocolOpened => ProtocolOpened
					PeerState::ProtocolOpened {
						connection_id,
						connected_point,
					} => {
						*entry = PeerState::ProtocolOpened {
							connection_id,
							connected_point,
						};
					}
					PeerState::Locked => unreachable!(),
				}
			}
			HandlerOut::ProtocolClose { reason } => {
				let mut entry = self
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
						connection_id,
						connected_point,
					} => {
						*entry = PeerState::Connected {
							connection_id,
							connected_point,
						};
					}
					// ProtocolOpened => Connected
					PeerState::ProtocolOpened {
						connection_id,
						connected_point,
					} => {
						if conn == connection_id {
							debug!("Handler({}, {:?}) => ProtocolClose", source, conn);
							debug!(
								"External <= ProtocolClose({}): Through: {:?}",
								source, connected_point
							);
							self.events.push_back(NetworkBehaviourAction::GenerateEvent(
								ProtocolOut::ProtocolClose {
									peer_id: source,
									connected_point: connected_point.clone(),
								},
							));
							*entry = PeerState::Connected {
								connection_id,
								connected_point,
							};
						} else {
							*entry = PeerState::ProtocolOpened {
								connection_id,
								connected_point,
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
				let mut entry = self
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
						connection_id,
						connected_point,
					} => {
						if conn == connection_id {
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
							}
						}
						*entry = PeerState::Connected {
							connection_id,
							connected_point,
						};
					}
					// ProtocolOpened => ProtocolOpened
					PeerState::ProtocolOpened {
						connection_id,
						connected_point,
					} => {
						if conn == connection_id {
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
							}
						}
						*entry = PeerState::ProtocolOpened {
							connection_id,
							connected_point,
						};
					}
					PeerState::Locked => unreachable!(),
				}
			}
			HandlerOut::Message { message } => {
				debug!("Handler({}, {:?}) => Message", source, conn);
				debug!("External <= Message({})", source);
				self.events.push_back(NetworkBehaviourAction::GenerateEvent(
					ProtocolOut::Message { message },
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
		let mut entry = self
			.peers
			.entry(peer_id.clone())
			.or_insert(PeerState::Init { pending_dial: None });
		match std::mem::replace(entry, PeerState::Locked) {
			// Init => Init
			PeerState::Init { pending_dial } => {
				debug!("Libp2p => Dial failure for {}", peer_id);
				debug!("PeerManager <= Dropped({})", peer_id);
				self.peer_manager.dropped(peer_id.clone());
				self.delay_peers
					.insert(peer_id.clone(), Instant::now() + DELAY);
				*entry = PeerState::Init { pending_dial: None };
			}
			// Connected => Connected
			PeerState::Connected {
				connection_id,
				connected_point,
			} => {
				*entry = PeerState::Connected {
					connection_id,
					connected_point,
				};
			}
			// ProtocolOpened => ProtocolOpened
			PeerState::ProtocolOpened {
				connection_id,
				connected_point,
			} => {
				*entry = PeerState::ProtocolOpened {
					connection_id,
					connected_point,
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
				match pending_dial {
					Some(pd) => {
						match pd.poll_unpin(cx) {
							Poll::Ready(_) => {
								// dial
								debug!("Apply pending dial({})", peer_id);
								debug!("Libp2p <= Dial({})", peer_id);
								self.events.push_back(NetworkBehaviourAction::DialPeer {
									peer_id: peer_id.clone(),
									condition: DialPeerCondition::Disconnected,
								});
								std::mem::replace(pending_dial, None);
							}
							Poll::Pending => (),
						}
					}
					_ => (),
				}
			}
		}

		if let Some(event) = self.events.pop_front() {
			return Poll::Ready(event);
		}

		Poll::Pending
	}
}
