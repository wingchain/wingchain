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

use std::error;
use std::task::{Context, Poll};

use libp2p::core::connection::ConnectionId;
use libp2p::core::{ConnectedPoint, Multiaddr};
use libp2p::swarm::{NetworkBehaviour, NetworkBehaviourAction, PollParameters};
use libp2p::PeerId;

use crate::protocol::handler::{Handler, HandlerIn, HandlerOut, HandlerProto};
use std::borrow::Cow;

mod handler;
mod upgrade;

const PROTOCOL_NAME: &'static [u8] = b"/wingchain/1";

pub enum ProtocolOut {}

pub struct Protocol {}

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
	}

	fn inject_connection_closed(
		&mut self,
		peer_id: &PeerId,
		conn: &ConnectionId,
		_endpoint: &ConnectedPoint,
	) {
	}

	fn inject_event(&mut self, source: PeerId, connection: ConnectionId, event: HandlerOut) {}

	fn inject_addr_reach_failure(
		&mut self,
		peer_id: Option<&PeerId>,
		addr: &Multiaddr,
		error: &dyn error::Error,
	) {
	}

	fn inject_dial_failure(&mut self, peer_id: &PeerId) {}

	fn poll(
		&mut self,
		cx: &mut Context,
		_params: &mut impl PollParameters,
	) -> Poll<NetworkBehaviourAction<HandlerIn, Self::OutEvent>> {
		unimplemented!()
	}
}
