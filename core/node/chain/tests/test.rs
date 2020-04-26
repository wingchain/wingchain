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

use serde::Serialize;

use crypto::hash::{Hash as HashT, HashImpl};
use node_chain::{Chain, ChainConfig};
use node_db::DB;
use node_executor::{module, Executor};
use node_statedb::{StateDB, TrieRoot};
use primitives::{codec, Block, Body, DBKey, Executed, Hash, Header, Transaction};

#[test]
fn test_chain() {
	use tempfile::tempdir;

	env_logger::init();

	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	init(&home);

	let config = ChainConfig { home };

	let chain = Chain::new(config).unwrap();

	let best_number = chain.get_best_number().unwrap();

	assert_eq!(best_number, Some(0));

	let executed_number = chain.get_executed_number().unwrap();

	assert_eq!(executed_number, None);

	let (expected_block_hash, expected_block, expected_executed, expected_tx) = expected_data();

	let block_hash = chain.get_block_hash(&0).unwrap().unwrap();
	assert_eq!(block_hash, expected_block_hash);

	let header = chain.get_header(&block_hash).unwrap().unwrap();

	assert_eq!(header, expected_block.header.clone());

	let block = chain.get_block(&block_hash).unwrap().unwrap();

	assert_eq!(block, expected_block);

	let executed = chain.get_executed(&block_hash).unwrap().unwrap();

	assert_eq!(executed, expected_executed);

	let tx = chain
		.get_transaction(&block.body.meta_txs[0])
		.unwrap()
		.unwrap();

	assert_eq!(tx, expected_tx);
}

fn expected_data() -> (Hash, Block, Executed, Transaction) {
	let executor = Executor;

	let tx = executor
		.build_tx(
			"system".to_string(),
			"init".to_string(),
			module::system::InitParams {
				chain_id: "chain-test".to_string(),
				timestamp: 1587051962,
			},
		)
		.unwrap();

	let txs = vec![Arc::new(tx.clone())];

	let meta_txs_root = expected_txs_root(&txs);
	let meta_state_root = expected_meta_state_root(&txs);

	let payload_txs_root = expected_txs_root(&vec![]);

	let zero_hash = vec![0u8; 32];

	let header = Header {
		number: 0,
		timestamp: 1587051962,
		parent_hash: Hash(zero_hash.clone()),
		meta_txs_root,
		meta_state_root,
		payload_txs_root,
		payload_executed_gap: 1,
		payload_executed_state_root: Hash(zero_hash),
	};

	let block_hash = hash(&header);

	let meta_txs = txs.iter().map(|x| hash(&**x)).collect();
	let payload_txs = vec![];

	let block = Block {
		header,
		body: Body {
			meta_txs,
			payload_txs,
		},
	};

	let payload_state_root = expected_payload_state_root();

	let executed = Executed {
		payload_executed_state_root: payload_state_root,
	};

	(block_hash, block, executed, tx)
}

fn hash<E: Serialize>(data: E) -> Hash {
	let hasher = HashImpl::Blake2b256;
	let mut hash = vec![0u8; hasher.length().into()];
	hasher.hash(&mut hash, &codec::encode(&data).unwrap());
	Hash(hash)
}

fn expected_txs_root(txs: &Vec<Arc<Transaction>>) -> Hash {
	let trie_root = TrieRoot::new(Arc::new(HashImpl::Blake2b256)).unwrap();
	let txs = txs.iter().map(|x| codec::encode(&**x).unwrap());
	Hash(trie_root.calc_ordered_trie_root(txs))
}

fn expected_meta_state_root(txs: &Vec<Arc<Transaction>>) -> Hash {
	let tx = &txs[0]; // use the last tx
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

fn expected_payload_state_root() -> Hash {
	let data = HashMap::new();

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

fn init(home: &PathBuf) {
	let config_path = home.join("config");

	fs::create_dir_all(&config_path).unwrap();

	let spec = r#"
[basic]
hash = "blake2b_256"
dsa = "ed25519"
address = "blake2b_160"

[genesis]

# System module init
[[genesis.txs]]
method = "system.init"
params = ['''
{
    "chain_id": "chain-test",
    "time": "2020-04-16T23:46:02.189+08:00"
}
''']
	"#;

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
