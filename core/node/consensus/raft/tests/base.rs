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

use node_chain::{Chain, ChainConfig, DBConfig};
use node_consensus::Consensus;
use node_consensus_base::support::DefaultConsensusSupport;
use node_consensus_base::ConsensusConfig;
use node_coordinator::support::DefaultCoordinatorSupport;
use node_coordinator::{
	Coordinator, CoordinatorConfig, Keypair, LinkedHashMap, Multiaddr, NetworkConfig, PeerId,
	Protocol,
};
use node_txpool::support::DefaultTxPoolSupport;
use node_txpool::{TxPool, TxPoolConfig};
use primitives::BlockNumber;
use utils_test::TestAccount;

pub fn get_service(
	authority_accounts: &[&TestAccount],
	account: &TestAccount,
	local_key_pair: Keypair,
	port: u16,
	bootnodes: LinkedHashMap<(PeerId, Multiaddr), ()>,
) -> (
	Arc<Chain>,
	Arc<TxPool<DefaultTxPoolSupport>>,
	Arc<Consensus<DefaultConsensusSupport>>,
	Arc<Coordinator<DefaultCoordinatorSupport>>,
) {
	let chain = get_chain(authority_accounts);

	let txpool_config = TxPoolConfig { pool_capacity: 32 };

	let txpool_support = Arc::new(DefaultTxPoolSupport::new(chain.clone()));
	let txpool = Arc::new(TxPool::new(txpool_config, txpool_support).unwrap());

	let support = Arc::new(DefaultConsensusSupport::new(chain.clone(), txpool.clone()));

	let consensus_config = ConsensusConfig {
		secret_key: Some(account.secret_key.clone()),
	};

	let consensus = Arc::new(Consensus::new(consensus_config, support).unwrap());

	let coordinator_support = Arc::new(DefaultCoordinatorSupport::new(
		chain.clone(),
		txpool.clone(),
		consensus.clone(),
	));
	let coordinator = get_coordinator(local_key_pair, port, bootnodes, coordinator_support);

	(chain, txpool, consensus, coordinator)
}

pub async fn wait_txpool(txpool: &Arc<TxPool<DefaultTxPoolSupport>>, count: usize) {
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

pub async fn wait_block_execution(chain: &Arc<Chain>, expected_number: BlockNumber) {
	loop {
		{
			let number = chain.get_confirmed_number().unwrap().unwrap();
			let block_hash = chain.get_block_hash(&number).unwrap().unwrap();
			let execution = chain.get_execution(&block_hash).unwrap();
			if number == expected_number && execution.is_some() {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}
}

/// to avoid rocksdb `libc++abi.dylib: Pure virtual function called!`
#[allow(dead_code)]
pub async fn safe_close(
	chain: Arc<Chain>,
	txpool: Arc<TxPool<DefaultTxPoolSupport>>,
	consensus: Consensus<DefaultConsensusSupport>,
	coordinator: Arc<Coordinator<DefaultCoordinatorSupport>>,
) {
	drop(chain);
	drop(txpool);
	drop(consensus);
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
		handshake_builder: None,
	};
	let config = CoordinatorConfig { network_config };

	let coordinator = Coordinator::new(config, support).unwrap();
	Arc::new(coordinator)
}

fn get_chain(authority_accounts: &[&TestAccount]) -> Arc<Chain> {
	let path = tempdir().expect("Could not create a temp dir");
	let home = path.into_path();

	init(&home, authority_accounts);

	let db = DBConfig {
		memory_budget: 1 * 1024 * 1024,
		path: home.join("data").join("db"),
		partitions: vec![],
	};

	let chain_config = ChainConfig { home, db };

	let chain = Arc::new(Chain::new(chain_config).unwrap());

	chain
}

fn init(home: &PathBuf, authority_accounts: &[&TestAccount]) {
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
    "max_until_gap": 20,
    "max_execution_gap": 8,
    "consensus": "raft"
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
module = "raft"
method = "init"
params = '''
{{
    "block_interval": 3000,
	"heartbeat_interval": 100,
	"election_timeout_min": 500,
	"election_timeout_max": 1000,
	"authorities": {{
		"threshold": 2,
		"members": [
			["{}", 1],
			["{}", 1],
			["{}", 1]
		]
	}}
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
		authority_accounts[0].address,
		authority_accounts[0].address,
		authority_accounts[1].address,
		authority_accounts[2].address,
	);

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
