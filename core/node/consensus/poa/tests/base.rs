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
use node_txpool::support::DefaultTxPoolSupport;
use node_txpool::{TxPool, TxPoolConfig};
use primitives::{Address, Hash, PublicKey, SecretKey, Transaction};

pub fn get_service(
	account: &(SecretKey, PublicKey, KeyPairImpl, Address),
) -> (
	Arc<Chain>,
	Arc<TxPool<DefaultTxPoolSupport>>,
	Poa<DefaultConsensusSupport>,
) {
	let chain = get_chain(&account.3);

	let txpool_config = TxPoolConfig { pool_capacity: 32 };

	let txpool_support = Arc::new(DefaultTxPoolSupport::new(chain.clone()));
	let txpool = Arc::new(TxPool::new(txpool_config, txpool_support).unwrap());

	let support = Arc::new(DefaultConsensusSupport::new(chain.clone(), txpool.clone()));

	let poa_config = PoaConfig {
		secret_key: account.0.clone(),
	};

	let poa = Poa::new(poa_config, support).unwrap();

	(chain, txpool, poa)
}

pub async fn insert_tx(
	chain: &Arc<Chain>,
	txpool: &Arc<TxPool<DefaultTxPoolSupport>>,
	tx: Transaction,
) -> Hash {
	let tx_hash = chain.hash_transaction(&tx).unwrap();
	txpool.insert(tx).unwrap();
	tx_hash
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
	txpool: Arc<TxPool<DefaultTxPoolSupport>>,
	poa: Poa<DefaultConsensusSupport>,
) {
	drop(chain);
	drop(txpool);
	drop(poa);
	tokio::time::sleep(Duration::from_millis(50)).await;
}

fn get_chain(address: &Address) -> Arc<Chain> {
	let path = tempdir().expect("Could not create a temp dir");
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
    "max_until_gap": 20,
    "max_execution_gap": 8
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
    "block_interval": null,
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
