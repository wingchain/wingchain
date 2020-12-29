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

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use std::time::Duration;
use tempfile::tempdir;

use crypto::dsa::KeyPairImpl;
use node_chain::{Chain, ChainConfig};
use node_consensus::support::DefaultConsensusSupport;
use node_consensus_poa::{Poa, PoaConfig};
use node_network::{Network, NetworkConfig, PeerId, Multiaddr, Protocol, LinkedHashMap, Keypair};
use node_txpool::{TxPool, TxPoolConfig};
use primitives::{Address, Hash, PublicKey, SecretKey, Transaction};
use node_coordinator::{Coordinator, CoordinatorConfig};
use node_coordinator::support::DefaultCoordinatorSupport;

pub fn get_service(
	authority_account: &(SecretKey, PublicKey, Address),
	account: &(SecretKey, PublicKey, Address),
	local_key_pair: Keypair,
	port: u16,
	bootnodes: LinkedHashMap<(PeerId, Multiaddr), ()>,
) -> (
	Arc<Chain>,
	Arc<TxPool<Chain>>,
	Poa<DefaultConsensusSupport<Chain>>,
	Arc<Coordinator<DefaultCoordinatorSupport>>
) {
	let chain = get_chain(&authority_account.2);

	let txpool_config = TxPoolConfig {
		pool_capacity: 32,
		buffer_capacity: 32,
	};

	let txpool = Arc::new(TxPool::new(txpool_config, chain.clone()).unwrap());

	let support = Arc::new(DefaultConsensusSupport::new(chain.clone(), txpool.clone()));

	let poa_config = PoaConfig {
		secret_key: account.0.clone(),
	};

	let poa = Poa::new(poa_config, support).unwrap();

	let coordinator_support = Arc::new(DefaultCoordinatorSupport::new(chain.clone()));
	let coordinator = get_coordinator(local_key_pair, port, bootnodes, coordinator_support);

	(chain, txpool, poa, coordinator)
}

pub async fn insert_tx(chain: &Arc<Chain>, txpool: &Arc<TxPool<Chain>>, tx: Transaction) -> Hash {
	let tx_hash = chain.hash_transaction(&tx).unwrap();
	txpool.insert(tx).await.unwrap();
	tx_hash
}

pub async fn wait_txpool(txpool: &Arc<TxPool<Chain>>, count: usize) {
	loop {
		{
			let queue = txpool.get_queue().read();
			if queue.len() == count {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}
}

pub async fn wait_block_execution(chain: &Arc<Chain>) {
	loop {
		{
			let number = chain.get_confirmed_number().unwrap().unwrap();
			let block_hash = chain.get_block_hash(&number).unwrap().unwrap();
			let execution = chain.get_execution(&block_hash).unwrap();
			if execution.is_some() {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}
}

/// safe close,
/// to avoid rocksdb `libc++abi.dylib: Pure virtual function called!`
pub async fn safe_close(
	chain: Arc<Chain>,
	txpool: Arc<TxPool<Chain>>,
	poa: Poa<DefaultConsensusSupport<Chain>>,
	coordinator: Arc<Coordinator<DefaultCoordinatorSupport>>,
) {
	drop(chain);
	drop(txpool);
	drop(poa);
	drop(coordinator);
	tokio::time::sleep(Duration::from_millis(50)).await;
}

fn get_coordinator(
	local_key_pair: Keypair,
	port: u16,
	bootnodes: LinkedHashMap<(PeerId, Multiaddr), ()>,
	support: Arc<DefaultCoordinatorSupport>,
) -> Arc<Coordinator<DefaultCoordinatorSupport>> {

	let agent_version = "wingchain/1.0.0".to_string();
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
		handshake: vec![],
	};
	let config = CoordinatorConfig {
		network_config,
	};

	let coordinator = Coordinator::new(config, support).unwrap();
	Arc::new(coordinator)
}

fn get_chain(address: &Address) -> Arc<Chain> {
	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	init(&home, address);

	let chain_config = ChainConfig { home };

	let chain = Arc::new(Chain::new(chain_config).unwrap());

	chain
}

fn init(home: &PathBuf, address: &Address) {
	let config_path = home.join("config");

	fs::create_dir_all(&config_path).unwrap();

	let spec = format!(
		r#"
[basic]
hash = "blake2b_256"
dsa = "ed25519"
address = "blake2b_160"

[genesis]

[[genesis.txs]]
module = "system"
method = "init"
params = '''
{{
    "chain_id": "chain-test",
    "timestamp": "2020-04-29T15:51:36.502+08:00",
    "until_gap": 20
}}
'''

[[genesis.txs]]
module = "balance"
method = "init"
params = '''
{{
    "endow": [
    	["{}", 10]
    ]
}}
'''

[[genesis.txs]]
module = "poa"
method = "init"
params = '''
{{
    "block_interval": 3000,
    "authority": "{}"
}}
'''

[[genesis.txs]]
module = "contract"
method = "init"
params = '''
{{
}}
'''
	"#,
		address, address
	);

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
