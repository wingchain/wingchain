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
use crypto::hash::{Hash as HashT, HashImpl};
use node_chain::{Chain, ChainConfig};
use node_consensus::support::DefaultConsensusSupport;
use node_consensus_solo::Solo;
use node_executor::module;
use node_txpool::{TxPool, TxPoolConfig};
use primitives::codec::Decode;
use primitives::{Address, Hash};
use utils_test::test_accounts;

#[tokio::test]
async fn test_solo_contract_create() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

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

	let ori_code = get_code().to_vec();

	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"create".to_string(),
			module::contract::CreateParams {
				code: ori_code.clone(),
				init_pay_value: 0,
				init_method: "init".to_string(),
				init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	// block 1
	delay_for(Duration::from_millis(1000)).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);
	let version: Option<u32> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_version".to_string(),
			module::contract::GetVersionParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("version: {}", version.unwrap());
	assert_eq!(version, Some(1));

	let code: Option<Vec<u8>> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code".to_string(),
			module::contract::GetCodeParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	log::info!("code: {:?}", code);
	assert_eq!(code, Some(ori_code));

	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account1.3.clone(), 1)],
		})
	);
}

#[tokio::test]
async fn test_solo_contract_update_admin() {
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

	let ori_code = get_code().to_vec();

	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"create".to_string(),
			module::contract::CreateParams {
				code: ori_code.clone(),
				init_pay_value: 0,
				init_method: "init".to_string(),
				init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	// block 1
	delay_for(Duration::from_millis(1000)).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account1.3.clone(), 1)],
		})
	);

	// update admin
	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"update_admin".to_string(),
			module::contract::UpdateAdminParams {
				contract_address: contract_address.clone(),
				admin: module::contract::Admin {
					threshold: 2,
					members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
				},
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	// block 2
	delay_for(Duration::from_millis(1000)).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 2,
			members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
		})
	);

	// update admin
	let tx1 = chain
		.build_transaction(
			Some((account2.0.clone(), 0, 10)),
			"contract".to_string(),
			"update_admin".to_string(),
			module::contract::UpdateAdminParams {
				contract_address: contract_address.clone(),
				admin: module::contract::Admin {
					threshold: 1,
					members: vec![(account2.3.clone(), 1)],
				},
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	let tx2 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"update_admin_vote".to_string(),
			module::contract::UpdateAdminVoteParams {
				contract_address: contract_address.clone(),
				proposal_id: 2,
			},
		)
		.unwrap();
	let tx2_hash = chain.hash_transaction(&tx2).unwrap();
	txpool.insert(tx2).await.unwrap();

	// block 3
	delay_for(Duration::from_millis(1000)).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let tx2_receipt = chain.get_receipt(&tx2_hash).unwrap().unwrap();
	log::info!("tx2_result: {:x?}", tx2_receipt.result);
	let tx2_events = tx2_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx2_events: {:x?}", tx2_events);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account2.3.clone(), 1)],
		})
	);
}

#[tokio::test]
async fn test_solo_contract_update_code() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);
	let hasher = Arc::new(HashImpl::Blake2b256);

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

	let ori_code = get_code().to_vec();

	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"create".to_string(),
			module::contract::CreateParams {
				code: ori_code.clone(),
				init_pay_value: 0,
				init_method: "init".to_string(),
				init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	// block 1
	delay_for(Duration::from_millis(1000)).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let code: Option<Vec<u8>> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code".to_string(),
			module::contract::GetCodeParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	log::info!("code: {:?}", code);
	assert_eq!(code, Some(ori_code));

	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account1.3.clone(), 1)],
		})
	);

	// update admin
	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"update_admin".to_string(),
			module::contract::UpdateAdminParams {
				contract_address: contract_address.clone(),
				admin: module::contract::Admin {
					threshold: 2,
					members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
				},
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	// block 2
	delay_for(Duration::from_millis(1000)).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 2,
			members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
		})
	);

	// update code
	let new_code = get_code().to_vec();
	let tx1 = chain
		.build_transaction(
			Some((account2.0.clone(), 0, 10)),
			"contract".to_string(),
			"update_code".to_string(),
			module::contract::UpdateCodeParams {
				contract_address: contract_address.clone(),
				code: new_code.clone(),
			},
		)
		.unwrap();
	let tx1_hash = chain.hash_transaction(&tx1).unwrap();
	txpool.insert(tx1).await.unwrap();

	let tx2 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"update_code_vote".to_string(),
			module::contract::UpdateCodeVoteParams {
				contract_address: contract_address.clone(),
				proposal_id: 1,
			},
		)
		.unwrap();
	let tx2_hash = chain.hash_transaction(&tx2).unwrap();
	txpool.insert(tx2).await.unwrap();

	// block 3
	delay_for(Duration::from_millis(1000)).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let tx2_receipt = chain.get_receipt(&tx2_hash).unwrap().unwrap();
	log::info!("tx2_result: {:x?}", tx2_receipt.result);
	let tx2_events = tx2_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx2_events: {:x?}", tx2_events);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	let code: Option<Vec<u8>> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code".to_string(),
			module::contract::GetCodeParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	log::info!("code: {:?}", code);
	assert_eq!(code, Some(new_code.clone()),);

	let code_hash: Option<Hash> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code_hash".to_string(),
			module::contract::GetCodeHashParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	let expect_code_hash = {
		let mut out = vec![0; hasher.length().into()];
		hasher.hash(&mut out, &new_code);
		Hash(out)
	};
	log::info!("code_hash: {:?}", code_hash);
	assert_eq!(code_hash, Some(expect_code_hash),);
}

fn get_code() -> &'static [u8] {
	let code = include_bytes!(
		"../../../vm/contract-samples/hello-world/pkg/contract_samples_hello_world_bg.wasm"
	);
	code
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
