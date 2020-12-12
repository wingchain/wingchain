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

use std::task::{Context, Poll};

use libp2p::core::ConnectedPoint;
use libp2p::swarm::{
	IntoProtocolsHandler, KeepAlive, NegotiatedSubstream, ProtocolsHandler, ProtocolsHandlerEvent,
	ProtocolsHandlerUpgrErr, SubstreamProtocol,
};
use libp2p::{InboundUpgrade, OutboundUpgrade, PeerId};

use crate::protocol::upgrade::{InProtocol, OutProtocol};
use libp2p::swarm::protocols_handler::{InboundUpgradeSend, OutboundUpgradeSend};
use std::borrow::Cow;

pub struct HandlerProto {}

impl IntoProtocolsHandler for HandlerProto {
	type Handler = Handler;

	fn into_handler(
		self,
		remote_peer_id: &PeerId,
		connected_point: &ConnectedPoint,
	) -> Self::Handler {
		Handler {}
	}

	fn inbound_protocol(&self) -> <Self::Handler as ProtocolsHandler>::InboundProtocol {
		unimplemented!()
	}
}

pub enum HandlerIn {
	Open,
	Close,
}

pub enum HandlerOut {}

#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum HandlerError {}

pub struct Handler {}

impl ProtocolsHandler for Handler {
	type InEvent = HandlerIn;
	type OutEvent = HandlerOut;
	type Error = HandlerError;
	type InboundProtocol = InProtocol;
	type OutboundProtocol = OutProtocol;
	type InboundOpenInfo = ();
	type OutboundOpenInfo = ();

	fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
		let upgrade = InProtocol::new(Cow::Borrowed(b""));
		SubstreamProtocol::new(upgrade, ())
	}

	fn inject_fully_negotiated_inbound(
		&mut self,
		protocol: <Self::InboundProtocol as InboundUpgradeSend>::Output,
		info: Self::InboundOpenInfo,
	) {
	}

	fn inject_fully_negotiated_outbound(
		&mut self,
		protocol: <Self::OutboundProtocol as OutboundUpgradeSend>::Output,
		info: Self::OutboundOpenInfo,
	) {
	}

	fn inject_event(&mut self, event: HandlerIn) {}

	fn inject_dial_upgrade_error(
		&mut self,
		info: Self::OutboundOpenInfo,
		error: ProtocolsHandlerUpgrErr<<Self::OutboundProtocol as OutboundUpgradeSend>::Error>,
	) {
	}

	fn connection_keep_alive(&self) -> KeepAlive {
		unimplemented!()
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
		unimplemented!()
	}
}
