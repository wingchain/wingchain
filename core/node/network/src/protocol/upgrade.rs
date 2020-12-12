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
use std::future::Future;
use std::io;
use std::iter;
use std::pin::Pin;

use libp2p::core::UpgradeInfo;
use libp2p::swarm::protocols_handler::{InboundUpgradeSend, OutboundUpgradeSend};
use libp2p::swarm::NegotiatedSubstream;
use libp2p::InboundUpgrade;

pub struct InProtocol {
	protocol_name: Cow<'static, [u8]>,
}

pub struct InSubstream {}

impl InProtocol {
	pub fn new(protocol_name: Cow<'static, [u8]>) -> Self {
		Self { protocol_name }
	}
}

impl UpgradeInfo for InProtocol {
	type Info = Cow<'static, [u8]>;
	type InfoIter = iter::Once<Self::Info>;

	fn protocol_info(&self) -> Self::InfoIter {
		iter::once(self.protocol_name.clone())
	}
}

impl InboundUpgradeSend for InProtocol {
	type Output = InSubstream;
	type Error = io::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

	fn upgrade_inbound(self, socket: NegotiatedSubstream, info: Self::Info) -> Self::Future {
		unimplemented!()
	}
}

pub struct OutProtocol {
	protocol_name: Cow<'static, [u8]>,
}

pub struct OutSubstream {}

impl OutProtocol {
	pub fn new(protocol_name: Cow<'static, [u8]>) -> Self {
		Self { protocol_name }
	}
}

impl UpgradeInfo for OutProtocol {
	type Info = Cow<'static, [u8]>;
	type InfoIter = iter::Once<Self::Info>;

	fn protocol_info(&self) -> Self::InfoIter {
		iter::once(self.protocol_name.clone())
	}
}

impl OutboundUpgradeSend for OutProtocol {
	type Output = OutSubstream;
	type Error = io::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

	fn upgrade_outbound(self, socket: NegotiatedSubstream, info: Self::Info) -> Self::Future {
		unimplemented!()
	}
}
