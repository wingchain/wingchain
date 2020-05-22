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

use tempfile::tempdir;
use tokio::time::Duration;

use node_chain::{module, Chain, ChainConfig};
use node_txpool::{PoolTransaction, TxPool, TxPoolConfig};
use utils_test::test_accounts;

#[tokio::test]
async fn test_txpool() {
	let chain = get_chain();
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, chain.clone()).unwrap();

	let (account1, account2) = test_accounts(
		chain.get_basic().dsa.clone(),
		chain.get_basic().address.clone(),
	);

	let tx = chain
		.build_transaction(
			Some((account1.0, 0, 1)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();

	let expected_queue = vec![Arc::new(PoolTransaction {
		tx: Arc::new(tx.clone()),
		tx_hash: chain.hash_transaction(&tx.clone()).unwrap(),
	})];

	txpool.insert(tx).await.unwrap();

	loop {
		{
			let queue = txpool.get_queue().read();
			if queue.len() > 0 {
				assert_eq!(*queue, expected_queue);
				break;
			}
		}
		tokio::time::delay_for(Duration::from_millis(10)).await;
	}
}

#[tokio::test]
async fn test_txpool_dup() {
	let chain = get_chain();
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, chain.clone()).unwrap();

	let (account1, account2) = test_accounts(
		chain.get_basic().dsa.clone(),
		chain.get_basic().address.clone(),
	);

	let tx = chain
		.build_transaction(
			Some((account1.0, 0, 1)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();

	txpool.insert(tx.clone()).await.unwrap();

	let result = txpool.insert(tx).await;
	assert!(format!("{}", result.unwrap_err()).contains("Error: Duplicated tx"));
}

#[tokio::test]
async fn test_txpool_validate() {
	let chain = get_chain();
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, chain.clone()).unwrap();

	let (account1, account2) = test_accounts(
		chain.get_basic().dsa.clone(),
		chain.get_basic().address.clone(),
	);

	let mut tx = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 1)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();
	tx.call.module = "unknown".to_string();

	let result = txpool.insert(tx.clone()).await;
	assert!(format!("{}", result.unwrap_err()).contains("Error: Invalid tx witness"));


	let tx = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 21)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();
	let result = txpool.insert(tx).await;
	assert!(format!("{}", result.unwrap_err()).contains("Error: Invalid until"));

	let tx = chain
		.build_transaction(
			Some((account1.0, 0, 0)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3,
				value: 2,
			},
		)
		.unwrap();
	let result = txpool.insert(tx).await;
	assert!(format!("{}", result.unwrap_err()).contains("Error: Exceed until"));
}

#[tokio::test]
async fn test_txpool_capacity() {
	let chain = get_chain();
	let config = TxPoolConfig {
		pool_capacity: 2,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, chain.clone()).unwrap();

	let (account1, account2) = test_accounts(
		chain.get_basic().dsa.clone(),
		chain.get_basic().address.clone(),
	);

	let tx = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 1)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();

	let tx2 = chain
		.build_transaction(
			Some((account1.0.clone(), 1, 1)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();

	let tx3 = chain
		.build_transaction(
			Some((account1.0.clone(), 2, 1)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();

	txpool.insert(tx).await.unwrap();
	txpool.insert(tx2).await.unwrap();
	let result = txpool.insert(tx3).await;
	assert!(format!("{}", result.unwrap_err()).contains("Error: Exceed capacity"));
}

fn get_chain() -> Arc<Chain> {
	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	init(&home);

	let chain_config = ChainConfig { home };

	let chain = Arc::new(Chain::new(chain_config).unwrap());
	chain
}

fn init(home: &PathBuf) {
	let config_path = home.join("config");

	fs::create_dir_all(&config_path).unwrap();

	let spec = r#"
[basic]
hash = "blake2b_256"
dsa = "ed25519"
address = "blake2b_160"

[genesis]

[[genesis.txs]]
module = "system"
method = "init"
params = '''
{
    "chain_id": "chain-test",
    "timestamp": "2020-04-29T15:51:36.502+08:00",
    "until_gap": 20
}
'''

[[genesis.txs]]
module = "balance"
method = "init"
params = '''
{
    "endow": [
    	["b4decd5a5f8f2ba708f8ced72eec89f44f3be96a", 10]
    ]
}
'''
	"#;

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
