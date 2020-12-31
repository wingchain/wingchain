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

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use node_chain::{module, Chain, ChainConfig};
use node_txpool::support::DefaultTxPoolSupport;
use node_txpool::{TxPool, TxPoolConfig};
use primitives::{Address, FullTransaction};
use utils_test::test_accounts;

#[tokio::test]
async fn test_txpool() {
	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let chain = get_chain(&account1.3);
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool_support = Arc::new(DefaultTxPoolSupport::new(chain.clone()));
	let txpool = TxPool::new(config, txpool_support).unwrap();

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

	let expected_queue = vec![Arc::new(FullTransaction {
		tx: tx.clone(),
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
		tokio::time::sleep(Duration::from_millis(10)).await;
	}

	safe_close(chain, txpool).await;
}

#[tokio::test]
async fn test_txpool_dup() {
	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let chain = get_chain(&account1.3);
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool_support = Arc::new(DefaultTxPoolSupport::new(chain.clone()));
	let txpool = TxPool::new(config, txpool_support).unwrap();

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

	safe_close(chain, txpool).await;
}

#[tokio::test]
async fn test_txpool_validate() {
	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let chain = get_chain(&account1.3);
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool_support = Arc::new(DefaultTxPoolSupport::new(chain.clone()));
	let txpool = TxPool::new(config, txpool_support).unwrap();

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
	assert!(format!("{}", result.unwrap_err()).contains("Error: Exceed until"));

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
	assert!(format!("{}", result.unwrap_err()).contains("Error: Invalid until"));

	safe_close(chain, txpool).await;
}

#[tokio::test]
async fn test_txpool_capacity() {
	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let chain = get_chain(&account1.3);
	let config = TxPoolConfig {
		pool_capacity: 2,
		buffer_capacity: 256,
	};
	let txpool_support = Arc::new(DefaultTxPoolSupport::new(chain.clone()));
	let txpool = TxPool::new(config, txpool_support).unwrap();

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

	safe_close(chain, txpool).await;
}

/// safe close,
/// to avoid rocksdb `libc++abi.dylib: Pure virtual function called!`
async fn safe_close(chain: Arc<Chain>, txpool: TxPool<DefaultTxPoolSupport>) {
	drop(chain);
	drop(txpool);
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
	"#,
		address
	);

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
