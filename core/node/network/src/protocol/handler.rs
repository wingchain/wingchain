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

use log::trace;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::error;
use std::fmt;
use std::task::{Context, Poll};
use std::time::Duration;

use std::sync::Arc;
use futures::FutureExt;
use futures::StreamExt;
use futures_codec::BytesMut;
use futures_timer::Delay;
use libp2p::core::ConnectedPoint;
use libp2p::swarm::protocols_handler::{InboundUpgradeSend, OutboundUpgradeSend};
use libp2p::swarm::{
	IntoProtocolsHandler, KeepAlive, ProtocolsHandler, ProtocolsHandlerEvent,
	ProtocolsHandlerUpgrErr, SubstreamProtocol,
};
use libp2p::PeerId;

use crate::protocol::upgrade::{InProtocol, InSubstream, OutProtocol, OutSubstream};
use std::fmt::Formatter;

const OPEN_TIMEOUT: Duration = Duration::from_secs(20);

pub struct HandlerProto {
	local_peer_id: PeerId,
	protocol_name: Cow<'static, [u8]>,
	handshake: Arc<Vec<u8>>,
}

impl HandlerProto {
	pub fn new(
		local_peer_id: PeerId,
		protocol_name: Cow<'static, [u8]>,
		handshake: Arc<Vec<u8>>,
	) -> Self {
		Self {
			local_peer_id,
			protocol_name,
			handshake,
		}
	}
}

impl IntoProtocolsHandler for HandlerProto {
	type Handler = Handler;

	fn into_handler(
		self,
		remote_peer_id: &PeerId,
		connected_point: &ConnectedPoint,
	) -> Self::Handler {
		Handler {
			local_peer_id: self.local_peer_id.clone(),
			remote_peer_id: remote_peer_id.clone(),
			connected_point: connected_point.clone(),
			protocol_name: self.protocol_name,
			handshake: self.handshake,
			state: State::Init,
			events_queue: VecDeque::with_capacity(16),
		}
	}

	fn inbound_protocol(&self) -> <Self::Handler as ProtocolsHandler>::InboundProtocol {
		InProtocol::new(self.protocol_name.clone())
	}
}

#[derive(Clone)]
pub enum HandlerIn {
	Open,
	Close,
	SendMessage { message: Vec<u8> },
}

pub enum State {
	Init,
	Opening {
		in_substream: Option<InSubstream>,
		out_substream: Option<OutSubstream>,
		deadline: Delay,
	},
	Opened {
		in_substream: InSubstream,
		out_substream: OutSubstream,
	},
	Closed,
	Locked,
}

pub enum HandlerOut {
	ProtocolOpen {
		handshake: Vec<u8>,
	},
	ProtocolClose {
		reason: Cow<'static, str>,
	},
	ProtocolError {
		should_disconnect: bool,
		error: Box<dyn error::Error + Send + Sync>,
	},
	Message {
		message: BytesMut,
	},
}

#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum HandlerError {}

pub struct Handler {
	local_peer_id: PeerId,
	#[allow(dead_code)]
	remote_peer_id: PeerId,
	connected_point: ConnectedPoint,
	protocol_name: Cow<'static, [u8]>,
	handshake: Arc<Vec<u8>>,
	state: State,
	events_queue: VecDeque<ProtocolsHandlerEvent<OutProtocol, (), HandlerOut, HandlerError>>,
}

impl Handler {
	fn open(&mut self) {
		self.state = match std::mem::replace(&mut self.state, State::Locked) {
			State::Init => {
				let upgrade = OutProtocol::new(self.protocol_name.clone(), self.handshake.clone());
				self.events_queue
					.push_back(ProtocolsHandlerEvent::OutboundSubstreamRequest {
						protocol: SubstreamProtocol::new(upgrade, ()),
					});
				State::Opening {
					in_substream: None,
					out_substream: None,
					deadline: Delay::new(OPEN_TIMEOUT),
				}
			}
			State::Opening {
				in_substream,
				out_substream,
				..
			} => {
				let upgrade = OutProtocol::new(self.protocol_name.clone(), self.handshake.clone());
				self.events_queue
					.push_back(ProtocolsHandlerEvent::OutboundSubstreamRequest {
						protocol: SubstreamProtocol::new(upgrade, ()),
					});
				State::Opening {
					in_substream,
					out_substream,
					deadline: Delay::new(OPEN_TIMEOUT),
				}
			}
			State::Opened {
				in_substream,
				out_substream,
			} => State::Opened {
				in_substream,
				out_substream,
			},
			State::Closed => State::Closed,
			State::Locked => unreachable!(),
		};
	}

	fn close(&mut self) {
		self.state = match std::mem::replace(&mut self.state, State::Locked) {
			State::Init => State::Closed,
			State::Opening { .. } => State::Closed,
			State::Opened { .. } => {
				self.events_queue.push_back(ProtocolsHandlerEvent::Custom(
					HandlerOut::ProtocolClose {
						reason: "Closed by handler".into(),
					},
				));
				State::Closed
			}
			State::Closed => State::Closed,
			State::Locked => unreachable!(),
		};
	}

	fn send_message(&mut self, message: Vec<u8>) {
		match &mut self.state {
			State::Opened { out_substream, .. } => {
				out_substream.send_message(message);
			}
			_ => {
				self.events_queue.push_back(ProtocolsHandlerEvent::Custom(
					HandlerOut::ProtocolError {
						should_disconnect: false,
						error: "Send message when not opened".into(),
					},
				));
			}
		}
	}
}

impl ProtocolsHandler for Handler {
	type InEvent = HandlerIn;
	type OutEvent = HandlerOut;
	type Error = HandlerError;
	type InboundProtocol = InProtocol;
	type OutboundProtocol = OutProtocol;
	type InboundOpenInfo = ();
	type OutboundOpenInfo = ();

	fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
		let upgrade = InProtocol::new(self.protocol_name.clone());
		SubstreamProtocol::new(upgrade, ())
	}

	fn inject_fully_negotiated_inbound(
		&mut self,
		mut protocol: <Self::InboundProtocol as InboundUpgradeSend>::Output,
		_info: Self::InboundOpenInfo,
	) {
		trace!(
			"inject_fully_negotiated_inbound: \
		state: {:?}, \
		local_peer_id: {}, \
		remote_peer_id: {}, \
		connected_point: {:?}",
			self.state,
			self.local_peer_id,
			self.remote_peer_id,
			self.connected_point
		);
		self.state = match std::mem::replace(&mut self.state, State::Locked) {
			State::Init => State::Opening {
				in_substream: Some(protocol),
				out_substream: None,
				deadline: Delay::new(OPEN_TIMEOUT),
			},
			State::Opening {
				out_substream,
				deadline,
				..
			} => match out_substream {
				Some(out_substream) => {
					let handshake = protocol.take_received_handshake().expect("qed");
					self.events_queue.push_back(ProtocolsHandlerEvent::Custom(
						HandlerOut::ProtocolOpen { handshake },
					));
					State::Opened {
						in_substream: protocol,
						out_substream,
					}
				}
				None => State::Opening {
					in_substream: Some(protocol),
					out_substream: None,
					deadline,
				},
			},
			State::Opened {
				in_substream,
				out_substream,
			} => State::Opened {
				in_substream,
				out_substream,
			},
			State::Closed => State::Closed,
			State::Locked => unreachable!(),
		};
	}

	fn inject_fully_negotiated_outbound(
		&mut self,
		protocol: <Self::OutboundProtocol as OutboundUpgradeSend>::Output,
		_info: Self::OutboundOpenInfo,
	) {
		trace!(
			"inject_fully_negotiated_outbound: \
		state: {:?}, \
		local_peer_id: {}, \
		remote_peer_id: {}, \
		connected_point: {:?}",
			self.state,
			self.local_peer_id,
			self.remote_peer_id,
			self.connected_point
		);
		self.state = match std::mem::replace(&mut self.state, State::Locked) {
			State::Init => State::Opening {
				in_substream: None,
				out_substream: Some(protocol),
				deadline: Delay::new(OPEN_TIMEOUT),
			},
			State::Opening {
				in_substream,
				deadline,
				..
			} => match in_substream {
				Some(mut in_substream) => {
					let handshake = in_substream.take_received_handshake().expect("qed");
					self.events_queue.push_back(ProtocolsHandlerEvent::Custom(
						HandlerOut::ProtocolOpen { handshake },
					));
					State::Opened {
						in_substream,
						out_substream: protocol,
					}
				}
				None => State::Opening {
					in_substream: None,
					out_substream: Some(protocol),
					deadline,
				},
			},
			State::Opened {
				in_substream,
				out_substream,
			} => State::Opened {
				in_substream,
				out_substream,
			},
			State::Closed => State::Closed,
			State::Locked => unreachable!(),
		};
	}

	fn inject_event(&mut self, event: HandlerIn) {
		match event {
			HandlerIn::Open => self.open(),
			HandlerIn::Close => self.close(),
			HandlerIn::SendMessage { message } => self.send_message(message),
		}
	}

	fn inject_dial_upgrade_error(
		&mut self,
		_info: Self::OutboundOpenInfo,
		error: ProtocolsHandlerUpgrErr<<Self::OutboundProtocol as OutboundUpgradeSend>::Error>,
	) {
		let should_disconnect = match error {
			ProtocolsHandlerUpgrErr::Upgrade(_) => true,
			_ => false,
		};
		let event = HandlerOut::ProtocolError {
			should_disconnect,
			error: Box::new(error),
		};
		self.events_queue
			.push_back(ProtocolsHandlerEvent::Custom(event));
	}

	fn connection_keep_alive(&self) -> KeepAlive {
		match self.state {
			State::Init | State::Opening { .. } | State::Opened { .. } => KeepAlive::Yes,
			_ => KeepAlive::No,
		}
	}

	fn poll(
		&mut self,
		cx: &mut Context,
	) -> Poll<
		ProtocolsHandlerEvent<
			Self::OutboundProtocol,
			Self::OutboundOpenInfo,
			Self::OutEvent,
			Self::Error,
		>,
	> {
		if let Some(event) = self.events_queue.pop_front() {
			return Poll::Ready(event);
		}

		match std::mem::replace(&mut self.state, State::Locked) {
			State::Init => self.state = State::Init,
			State::Opening {
				in_substream,
				out_substream,
				mut deadline,
			} => {
				match deadline.poll_unpin(cx) {
					Poll::Ready(_) => {
						deadline.reset(OPEN_TIMEOUT);
						self.state = State::Opening {
							in_substream,
							out_substream,
							deadline,
						};
						return Poll::Ready(ProtocolsHandlerEvent::Custom(
							HandlerOut::ProtocolError {
								should_disconnect: true,
								error: "Timeout when opening protocol".to_string().into(),
							},
						));
					}
					Poll::Pending => (),
				}
				self.state = State::Opening {
					in_substream,
					out_substream,
					deadline,
				};
			}
			State::Opened {
				mut in_substream,
				mut out_substream,
			} => {
				match out_substream.poll_next_unpin(cx) {
					Poll::Ready(Some(Err(e))) => {
						self.state = State::Closed;
						return Poll::Ready(ProtocolsHandlerEvent::Custom(
							HandlerOut::ProtocolClose {
								reason: format!("Outbound substream encountered error: {}", e)
									.into(),
							},
						));
					}
					Poll::Ready(None) => {
						self.state = State::Closed;
						return Poll::Ready(ProtocolsHandlerEvent::Custom(
							HandlerOut::ProtocolClose {
								reason: "Outbound substream closed by the remote".into(),
							},
						));
					}
					Poll::Pending => (),
					Poll::Ready(Some(Ok(_))) => (),
				}
				match in_substream.poll_next_unpin(cx) {
					Poll::Ready(Some(Err(e))) => {
						self.state = State::Closed;
						return Poll::Ready(ProtocolsHandlerEvent::Custom(
							HandlerOut::ProtocolClose {
								reason: format!("Inbound substream encountered error: {}", e)
									.into(),
							},
						));
					}
					Poll::Ready(None) => {
						self.state = State::Closed;
						return Poll::Ready(ProtocolsHandlerEvent::Custom(
							HandlerOut::ProtocolClose {
								reason: "Inbound substream closed by the remote".into(),
							},
						));
					}
					Poll::Pending => {
						self.state = State::Opened {
							in_substream,
							out_substream,
						};
					}
					Poll::Ready(Some(Ok(message))) => {
						self.state = State::Opened {
							in_substream,
							out_substream,
						};
						return Poll::Ready(ProtocolsHandlerEvent::Custom(HandlerOut::Message {
							message,
						}));
					}
				}
			}
			State::Closed => {
				self.state = State::Closed;
			}
			State::Locked => unreachable!(),
		};

		Poll::Pending
	}
}

impl fmt::Debug for State {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match &self {
			State::Init => write!(f, "Init"),
			State::Opening { .. } => write!(f, "Opening"),
			State::Opened { .. } => write!(f, "Opened"),
			State::Closed => write!(f, "Closed"),
			State::Locked => write!(f, "Locked"),
		}
	}
}
