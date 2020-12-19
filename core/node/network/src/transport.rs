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

use crate::errors;
use libp2p::bandwidth::BandwidthSinks;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::Boxed;
use libp2p::{bandwidth, core::upgrade, dns, identity, noise, tcp, websocket, PeerId, Transport};
use primitives::errors::CommonResult;
use std::sync::Arc;
use std::time::Duration;

pub fn build_transport(
	keypair: identity::Keypair,
) -> CommonResult<(Boxed<(PeerId, StreamMuxerBox)>, Arc<BandwidthSinks>)> {
	let transport = tcp::TokioTcpConfig::new();
	let transport = websocket::WsConfig::new(transport.clone()).or_transport(transport);
	let transport = dns::DnsConfig::new(transport)
		.map_err(|e| errors::ErrorKind::Transport(format!("{}", e)))?;

	let (transport, bandwidth) = bandwidth::BandwidthLogging::new(transport);

	let authenticate_config = {
		let noise_keypair = noise::Keypair::<noise::X25519Spec>::new()
			.into_authentic(&keypair)
			.expect("qed");
		let xx_config = noise::NoiseConfig::xx(noise_keypair);
		xx_config.into_authenticated()
	};

	let multiplex_config = {
		let mut yamux_config = libp2p::yamux::YamuxConfig::default();
		yamux_config.set_window_update_mode(libp2p::yamux::WindowUpdateMode::on_read());
		yamux_config
	};

	let transport = transport
		.upgrade(upgrade::Version::V1Lazy)
		.authenticate(authenticate_config)
		.multiplex(multiplex_config)
		.timeout(Duration::from_secs(20))
		.boxed();

	Ok((transport, bandwidth))
}
