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

use async_std::task;
use futures::channel::oneshot;
use log::info;
use node_network::{Network, NetworkConfig, NetworkInMessage, NetworkState};
use std::time::Duration;

#[async_std::test]
async fn test_network() {
	let _ = env_logger::try_init();

	let key_pair0 = Keypair::generate_ed25519();
	let key_pair1 = Keypair::generate_ed25519();
	let key_pair2 = Keypair::generate_ed25519();

	let port0 = 3209;
	let port1 = 3210;
	let port2 = 3211;

	let agent_version0 = "wingchain/1.0.0".to_string();
	let agent_version1 = "wingchain/1.0.0".to_string();
	let agent_version2 = "wingchain/1.0.0".to_string();

	let bootnodes = (
		key_pair0.public().into_peer_id(),
		Multiaddr::empty()
			.with(Protocol::Ip4([127, 0, 0, 1].into()))
			.with(Protocol::Tcp(port0)),
	);
	let bootnodes =
		std::iter::once((bootnodes, ())).collect::<LinkedHashMap<(PeerId, Multiaddr), ()>>();

	let network0 = generate_network(key_pair0, port0, bootnodes.clone(), agent_version0);
	let network1 = generate_network(key_pair1, port1, bootnodes.clone(), agent_version1);
	let network2 = generate_network(key_pair2, port2, bootnodes.clone(), agent_version2);

	task::sleep(Duration::from_secs(70)).await;

	let network_state0 = get_network_state(&network0).await;
	info!("network_state0: {:?}", network_state0);
	assert_eq!(network_state0.opened_peers.len(), 2);

	let network_state1 = get_network_state(&network1).await;
	info!("network_state1: {:?}", network_state1);
	assert_eq!(network_state1.opened_peers.len(), 2);

	let network_state2 = get_network_state(&network2).await;
	info!("network_state2: {:?}", network_state2);
	assert_eq!(network_state2.opened_peers.len(), 2);
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

fn generate_network(
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
		handshake: b"wingchain".to_vec(),
	};

	let network = Network::new(network_config).unwrap();

	network
}
