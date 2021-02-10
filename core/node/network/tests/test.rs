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

use libp2p::core::Multiaddr;
use libp2p::identity::Keypair;
use libp2p::multiaddr::Protocol;
use libp2p::PeerId;
use linked_hash_map::LinkedHashMap;

use futures::channel::oneshot;
use futures::future::{join, join3, select, Either};
use futures::StreamExt;
use log::info;
use node_network::{
	HandshakeBuilder, Network, NetworkConfig, NetworkInMessage, NetworkOutMessage, NetworkState,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_network_connect() {
	let _ = env_logger::try_init();

	let specs = vec![
		(
			Keypair::generate_ed25519(),
			1001,
			"wingchain/1.0.0".to_string(),
		),
		(
			Keypair::generate_ed25519(),
			1002,
			"wingchain/1.0.0".to_string(),
		),
		(
			Keypair::generate_ed25519(),
			1003,
			"wingchain/1.0.0".to_string(),
		),
	];

	let bootnodes = {
		let bootnodes_spec = &specs[0];
		let bootnodes = (
			bootnodes_spec.0.public().into_peer_id(),
			Multiaddr::empty()
				.with(Protocol::Ip4([127, 0, 0, 1].into()))
				.with(Protocol::Tcp(bootnodes_spec.1)),
		);
		let bootnodes =
			std::iter::once((bootnodes, ())).collect::<LinkedHashMap<(PeerId, Multiaddr), ()>>();
		bootnodes
	};

	for (key_pair, ..) in &specs {
		info!("peer id: {}", key_pair.public().into_peer_id());
	}

	let mut networks = specs
		.iter()
		.map(|x| start_network(x.0.clone(), x.1.clone(), bootnodes.clone(), x.2.clone()))
		.collect::<Vec<_>>();

	// out messages
	for network in &mut networks {
		let mut rx = network.network_rx().unwrap();
		tokio::spawn(async move {
			loop {
				let message = rx.next().await;
				match message {
					Some(message) => {
						info!("Network get message: {:?}", message);
					}
					None => break,
				}
			}
		});
	}

	let wait_all = join3(
		wait_connect(&networks[0], 2),
		wait_connect(&networks[1], 2),
		wait_connect(&networks[2], 2),
	);
	futures::pin_mut!(wait_all);
	let wait_all = select(wait_all, futures_timer::Delay::new(Duration::from_secs(60))).await;
	match wait_all {
		Either::Left(_) => (),
		Either::Right(_) => panic!("Wait connect timeout"),
	}

	for network in &networks {
		let network_state = get_network_state(network).await;
		info!("Network state: {:?}", network_state);
	}

	// drop peer
	let tx = networks[0].network_tx();
	tx.unbounded_send(NetworkInMessage::DropPeer {
		peer_id: specs[1].0.public().into_peer_id(),
		delay: Some(Duration::from_secs(30)),
	})
	.unwrap();

	let wait_all = join(wait_connect(&networks[0], 1), wait_connect(&networks[1], 1));
	futures::pin_mut!(wait_all);
	let wait_all = select(wait_all, futures_timer::Delay::new(Duration::from_secs(60))).await;
	match wait_all {
		Either::Left(_) => (),
		Either::Right(_) => panic!("Wait connect timeout"),
	}
}

#[tokio::test]
async fn test_network_message() {
	let _ = env_logger::try_init();

	let specs = vec![
		(
			Keypair::generate_ed25519(),
			1004,
			"wingchain/1.0.0".to_string(),
		),
		(
			Keypair::generate_ed25519(),
			1005,
			"wingchain/1.0.0".to_string(),
		),
	];

	let bootnodes = {
		let bootnodes_spec = &specs[0];
		let bootnodes = (
			bootnodes_spec.0.public().into_peer_id(),
			Multiaddr::empty()
				.with(Protocol::Ip4([127, 0, 0, 1].into()))
				.with(Protocol::Tcp(bootnodes_spec.1)),
		);
		let bootnodes =
			std::iter::once((bootnodes, ())).collect::<LinkedHashMap<(PeerId, Multiaddr), ()>>();
		bootnodes
	};

	for (key_pair, ..) in &specs {
		info!("peer id: {}", key_pair.public().into_peer_id());
	}

	let mut networks = specs
		.iter()
		.map(|x| start_network(x.0.clone(), x.1.clone(), bootnodes.clone(), x.2.clone()))
		.collect::<Vec<_>>();

	let wait_all = join(wait_connect(&networks[0], 1), wait_connect(&networks[1], 1));

	wait_all.await;

	for network in &networks {
		let network_state = get_network_state(network).await;
		info!("Network state: {:?}", network_state);
	}

	let network0_tx = &networks[0].network_tx();
	let network1_peer_id = specs[1].0.public().into_peer_id();
	network0_tx
		.unbounded_send(NetworkInMessage::SendMessage {
			peer_id: network1_peer_id,
			message: b"hello".to_vec(),
		})
		.unwrap();

	let wait_message = wait_message(&mut networks[1], b"hello");
	futures::pin_mut!(wait_message);
	let wait_message = select(
		wait_message,
		futures_timer::Delay::new(Duration::from_secs(60)),
	)
	.await;

	match wait_message {
		Either::Left(_) => (),
		Either::Right(_) => panic!("Wait message timeout"),
	}
}

fn start_network(
	local_key_pair: Keypair,
	port: u16,
	bootnodes: LinkedHashMap<(PeerId, Multiaddr), ()>,
	agent_version: String,
) -> Network {
	let listen_address = Multiaddr::empty()
		.with(Protocol::Ip4([0, 0, 0, 0].into()))
		.with(Protocol::Tcp(port));
	let listen_addresses = vec![listen_address].into_iter().map(|v| (v, ())).collect();
	let network_config = NetworkConfig {
		max_in_peers: 32,
		max_out_peers: 32,
		listen_addresses,
		external_addresses: LinkedHashMap::new(),
		bootnodes,
		reserved_nodes: LinkedHashMap::new(),
		reserved_only: false,
		agent_version,
		local_key_pair,
		handshake_builder: Some(Arc::new(DummyHandshakeBuilder)),
	};

	let network = Network::new(network_config).unwrap();
	network
}

async fn wait_connect(network: &Network, expected_opened_count: usize) {
	loop {
		let network_state = get_network_state(network).await;
		if network_state.opened_peers.len() >= expected_opened_count {
			break;
		}
		tokio::time::sleep(Duration::from_millis(1000)).await;
	}
}

async fn wait_message(network: &mut Network, expected_message: &[u8]) {
	let mut rx = network.network_rx().unwrap();
	loop {
		match rx.next().await {
			Some(message) => match message {
				NetworkOutMessage::Message { peer_id, message }
					if message.as_ref() == expected_message =>
				{
					info!(
						"Network get message: peer_id: {}, message: {:?}",
						peer_id, message
					);
					break;
				}
				_ => (),
			},
			None => break,
		}
		tokio::time::sleep(Duration::from_millis(1000)).await;
	}
}

async fn get_network_state(network: &Network) -> NetworkState {
	let network_tx = network.network_tx();
	let (tx, rx) = oneshot::channel();
	network_tx
		.unbounded_send(NetworkInMessage::GetNetworkState { tx })
		.unwrap();
	let network_state = rx.await.unwrap();
	network_state
}

struct DummyHandshakeBuilder;
impl HandshakeBuilder for DummyHandshakeBuilder {
	fn build(&self, _nonce: u64) -> Vec<u8> {
		vec![]
	}
}
