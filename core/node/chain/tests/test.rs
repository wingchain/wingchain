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

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use log::info;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::{Hash as HashT, HashImpl};
use node_chain::{module, Chain, ChainConfig};
use node_db::DB;
use node_statedb::{StateDB, TrieRoot};
use primitives::codec::Encode;
use primitives::types::FullReceipt;
use primitives::{
	codec, Address, Balance, Block, Body, DBKey, Executed, FullTransaction, Hash, Header, Receipt,
	TransactionForHash,
};
use utils_test::test_accounts;

#[tokio::test]
async fn test_chain() {
	env_logger::init();

	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	init(&home, &account1.3);

	let config = ChainConfig { home };

	let chain = Chain::new(config).unwrap();

	let (
		expected_block_hash,
		expected_block,
		expected_executed,
		expected_meta_txs,
		expected_payload_txs,
		expected_meta_receipts,
		expected_payload_receipts,
	) = expected_data(&chain, &account1.3);

	// confirmed number
	let confirmed_number = chain.get_confirmed_number().unwrap();

	assert_eq!(confirmed_number, Some(0));

	// confirmed executed number
	let confirmed_executed_number = chain.get_confirmed_executed_number().unwrap();

	assert_eq!(confirmed_executed_number, None);

	// block hash
	let block_hash = chain.get_block_hash(&0).unwrap().unwrap();

	info!("Block hash: {:?}", block_hash);

	assert_eq!(block_hash, expected_block_hash);

	// header
	let header = chain.get_header(&block_hash).unwrap().unwrap();

	assert_eq!(header, expected_block.header.clone());

	// block
	let block = chain.get_block(&block_hash).unwrap().unwrap();

	info!("Block: {:?}", block);

	assert_eq!(block, expected_block);

	// executed
	let executed = chain.get_executed(&block_hash).unwrap().unwrap();

	assert_eq!(executed, expected_executed);

	// meta tx
	let meta_tx_hash = &block.body.meta_txs[0];
	let meta_tx = chain.get_transaction(&meta_tx_hash).unwrap().unwrap();
	let meta_tx = FullTransaction {
		tx_hash: meta_tx_hash.clone(),
		tx: meta_tx,
	};

	assert_eq!(vec![Arc::new(meta_tx)], expected_meta_txs);

	// meta tx receipt
	let meta_tx_receipt = chain.get_receipt(&meta_tx_hash).unwrap().unwrap();
	let meta_tx_receipt = FullReceipt {
		tx_hash: meta_tx_hash.clone(),
		receipt: meta_tx_receipt,
	};

	assert_eq!(vec![Arc::new(meta_tx_receipt)], expected_meta_receipts);

	// payload tx
	let payload_tx_hash = &block.body.payload_txs[0];
	let payload_tx = chain.get_transaction(&payload_tx_hash).unwrap().unwrap();
	let payload_tx = FullTransaction {
		tx_hash: payload_tx_hash.clone(),
		tx: payload_tx,
	};

	assert_eq!(vec![Arc::new(payload_tx)], expected_payload_txs);

	// payload tx receipt
	let payload_tx_receipt = chain.get_receipt(&payload_tx_hash).unwrap().unwrap();
	let payload_tx_receipt = FullReceipt {
		tx_hash: payload_tx_hash.clone(),
		receipt: payload_tx_receipt,
	};

	assert_eq!(
		vec![Arc::new(payload_tx_receipt)],
		expected_payload_receipts
	);
}

#[tokio::test]
async fn test_chain_execute_call() {
	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	init(&home, &account1.3);

	let config = ChainConfig { home };

	let chain = Chain::new(config).unwrap();

	let block_hash = chain.get_block_hash(&0).unwrap().unwrap();

	let sender = account1.3;
	let params = node_executor_primitives::EmptyParams;
	let call = chain
		.build_transaction(
			None,
			"balance".to_string(),
			"get_balance".to_string(),
			params,
		)
		.unwrap()
		.call;
	let result = chain
		.execute_call(&block_hash, Some(&sender), &call)
		.unwrap()
		.unwrap();
	let result: Balance = codec::decode(&result).unwrap();
	assert_eq!(10, result);
}

#[test]
fn test_chain_invalid_spec() {
	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	init_invalid_spec(&home);

	let config = ChainConfig { home };

	let chain = Chain::new(config);

	match chain {
		Err(e) => {
			assert!(format!("{}", e).contains("Error: Invalid address"));
		}
		Ok(_) => unreachable!(),
	}
}

fn expected_data(
	chain: &Chain,
	account: &Address,
) -> (
	Hash,
	Block,
	Executed,
	Vec<Arc<FullTransaction>>,
	Vec<Arc<FullTransaction>>,
	Vec<Arc<FullReceipt>>,
	Vec<Arc<FullReceipt>>,
) {
	let timestamp = 1588146696502;

	let meta_txs = vec![chain
		.build_transaction(
			None,
			"system".to_string(),
			"init".to_string(),
			module::system::InitParams {
				chain_id: "chain-test".to_string(),
				timestamp,
				until_gap: 20,
			},
		)
		.unwrap()];

	let meta_txs = meta_txs
		.into_iter()
		.map(|tx| {
			let tx_hash = chain.hash_transaction(&tx).unwrap();
			Arc::new(FullTransaction { tx, tx_hash })
		})
		.collect::<Vec<_>>();

	let meta_txs_root = expected_txs_root(&meta_txs);
	let (meta_receipts_root, meta_receipts) = expected_block_0_receipts_root(&meta_txs);
	let meta_state_root = expected_block_0_meta_state_root(&meta_txs);
	let meta_txs_hash = meta_txs.iter().map(|x| x.tx_hash.clone()).collect();

	let payload_txs = vec![chain
		.build_transaction(
			None,
			"balance".to_string(),
			"init".to_string(),
			module::balance::InitParams {
				endow: vec![(account.clone(), 10)],
			},
		)
		.unwrap()];

	let payload_txs = payload_txs
		.into_iter()
		.map(|tx| {
			let tx_hash = chain.hash_transaction(&tx).unwrap();
			Arc::new(FullTransaction { tx, tx_hash })
		})
		.collect::<Vec<_>>();

	let payload_txs_root = expected_txs_root(&payload_txs);
	let (payload_receipts_root, payload_receipts) = expected_block_0_receipts_root(&payload_txs);
	let payload_state_root = expected_block_0_payload_state_root(&payload_txs);
	let payload_txs_hash = payload_txs.iter().map(|x| x.tx_hash.clone()).collect();

	let zero_hash = vec![0u8; 32];

	let header = Header {
		number: 0,
		timestamp,
		parent_hash: Hash(zero_hash.clone()),
		meta_txs_root,
		meta_state_root,
		meta_receipts_root,
		payload_txs_root,
		payload_executed_gap: 1,
		payload_executed_state_root: Hash(zero_hash.clone()),
		payload_executed_receipts_root: Hash(zero_hash),
	};

	let block_hash = hash(&header);

	let block = Block {
		header,
		body: Body {
			meta_txs: meta_txs_hash,
			payload_txs: payload_txs_hash,
		},
	};

	let executed = Executed {
		payload_executed_state_root: payload_state_root,
		payload_executed_receipts_root: payload_receipts_root,
	};

	(
		block_hash,
		block,
		executed,
		meta_txs,
		payload_txs,
		meta_receipts,
		payload_receipts,
	)
}

fn hash<E: Encode>(data: E) -> Hash {
	let hasher = HashImpl::Blake2b256;
	let mut hash = vec![0u8; hasher.length().into()];
	hasher.hash(&mut hash, &codec::encode(&data).unwrap());
	Hash(hash)
}

fn expected_txs_root(txs: &Vec<Arc<FullTransaction>>) -> Hash {
	let trie_root = TrieRoot::new(Arc::new(HashImpl::Blake2b256)).unwrap();
	let txs = txs
		.into_iter()
		.map(|x| codec::encode(&TransactionForHash::new(&x.tx)).unwrap());
	Hash(trie_root.calc_ordered_trie_root(txs))
}

fn expected_block_0_receipts_root(
	txs: &Vec<Arc<FullTransaction>>,
) -> (Hash, Vec<Arc<FullReceipt>>) {
	let trie_root = TrieRoot::new(Arc::new(HashImpl::Blake2b256)).unwrap();

	let receipts = txs
		.into_iter()
		.map(|x| {
			Arc::new(FullReceipt {
				tx_hash: x.tx_hash.clone(),
				receipt: Receipt {
					block_number: 0,
					events: vec![],
					result: Ok(codec::encode(&()).unwrap()),
				},
			})
		})
		.collect::<Vec<_>>();

	let map = receipts.iter().map(|x| codec::encode(&x.receipt).unwrap());
	let root = Hash(trie_root.calc_ordered_trie_root(map));

	(root, receipts)
}

fn expected_block_0_meta_state_root(txs: &Vec<Arc<FullTransaction>>) -> Hash {
	let tx = &txs[0].tx; // use the last tx
	let params: module::system::InitParams = codec::decode(&tx.call.params.0[..]).unwrap();

	let data = vec![
		(
			DBKey::from_slice(b"system_chain_id"),
			Some(codec::encode(&params.chain_id).unwrap()),
		),
		(
			DBKey::from_slice(b"system_timestamp"),
			Some(codec::encode(&params.timestamp).unwrap()),
		),
		(
			DBKey::from_slice(b"system_until_gap"),
			Some(codec::encode(&params.until_gap).unwrap()),
		),
	]
	.into_iter()
	.collect::<HashMap<_, _>>();

	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = Arc::new(DB::open(&path).unwrap());
	let hasher = Arc::new(HashImpl::Blake2b256);

	let statedb =
		Arc::new(StateDB::new(db.clone(), node_db::columns::META_STATE, hasher.clone()).unwrap());

	let (state_root, _) = statedb
		.prepare_update(&statedb.default_root(), data.iter())
		.unwrap();
	Hash(state_root)
}

fn expected_block_0_payload_state_root(txs: &Vec<Arc<FullTransaction>>) -> Hash {
	let tx = &txs[0].tx; // use the last tx
	let params: module::balance::InitParams = codec::decode(&tx.call.params.0[..]).unwrap();

	let (account, balance) = &params.endow[0];

	let data = vec![(
		DBKey::from_slice(
			&[
				&b"balance_balance_"[..],
				&codec::encode(&account.0).unwrap(),
			]
			.concat(),
		),
		Some(codec::encode(&balance).unwrap()),
	)]
	.into_iter()
	.collect::<HashMap<_, _>>();

	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = Arc::new(DB::open(&path).unwrap());
	let hasher = Arc::new(HashImpl::Blake2b256);

	let statedb = Arc::new(
		StateDB::new(db.clone(), node_db::columns::PAYLOAD_STATE, hasher.clone()).unwrap(),
	);

	let (state_root, _) = statedb
		.prepare_update(&statedb.default_root(), data.iter())
		.unwrap();
	Hash(state_root)
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
    "until_gap" : 20
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

fn init_invalid_spec(home: &PathBuf) {
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
    "until_gap" : 20
}
'''

[[genesis.txs]]
module = "balance"
method = "init"
params = '''
{
    "endow": [
    	["b4decd5a5f8f2ba708f8ced72eec89f44f3be96a00", 10]
    ]
}
'''
	"#;

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
