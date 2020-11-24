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
use std::time::SystemTime;

use tempfile::tempdir;
use tokio::time::{delay_for, Duration};

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use node_chain::{Chain, ChainConfig};
use node_consensus::support::DefaultConsensusSupport;
use node_consensus_solo::Solo;
use node_executor::module;
use node_txpool::{TxPool, TxPoolConfig};
use primitives::{codec, Address, Balance, Event, Receipt};
use utils_test::test_accounts;

#[tokio::test]
async fn test_solo_balance() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, account2) = test_accounts(dsa, address);

	let chain = get_chain(&account1.3);

	let txpool_config = TxPoolConfig {
		pool_capacity: 32,
		buffer_capacity: 32,
	};

	let txpool = Arc::new(TxPool::new(txpool_config, chain.clone()).unwrap());

	let support = Arc::new(DefaultConsensusSupport::new(chain.clone(), txpool.clone()));

	let _solo = Solo::new(support).unwrap();

	let delay_to_insert_tx = time_until_next(duration_now(), 1000) / 2;

	// block 0
	delay_for(delay_to_insert_tx).await;

	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 1,
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	// block 1
	delay_for(Duration::from_millis(1000)).await;

	let tx2 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 11)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();
	let tx2_hash = chain.hash_transaction(&tx2).unwrap();
	txpool.insert(tx2).await.unwrap();

	// block 2
	delay_for(Duration::from_millis(1000)).await;

	let tx3 = chain
		.build_transaction(
			Some((account1.0, 0, 12)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 3,
			},
		)
		.unwrap();
	let tx3_hash = chain.hash_transaction(&tx3).unwrap();
	txpool.insert(tx3).await.unwrap();

	delay_for(Duration::from_millis(1000)).await;

	// check

	let balance: Balance = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.3),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 9);

	let block1 = chain
		.get_block(&chain.get_block_hash(&1).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block1.body.payload_txs[0], tx1_hash);

	let balance: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.3),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 7);
	let block2 = chain
		.get_block(&chain.get_block_hash(&2).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block2.body.payload_txs[0], tx2_hash);

	let balance: Balance = chain
		.execute_call_with_block_number(
			&3,
			Some(&account1.3),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 4);
	let block3 = chain
		.get_block(&chain.get_block_hash(&3).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block3.body.payload_txs[0], tx3_hash);

	let tx3_receipt = chain.get_receipt(&tx3_hash).unwrap().unwrap();
	assert_eq!(
		tx3_receipt,
		Receipt {
			block_number: 3,
			events: vec![Event::from_data(
				"Transferred".to_string(),
				module::balance::Transferred {
					sender: account1.3,
					recipient: account2.3,
					value: 3,
				}
			)
			.unwrap()],
			result: Ok(codec::encode(&()).unwrap()),
		}
	);
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
module = "solo"
method = "init"
params = '''
{{
    "block_interval": 1000
}}
'''
	"#,
		address
	);

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}

fn time_until_next(now: Duration, duration: u64) -> Duration {
	let remaining_full_millis = duration - (now.as_millis() as u64 % duration) - 1;
	Duration::from_millis(remaining_full_millis)
}

fn duration_now() -> Duration {
	let now = SystemTime::now();
	now.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap_or_else(|e| {
			panic!(
				"Current time {:?} is before unix epoch. Something is wrong: {:?}",
				now, e,
			)
		})
}
