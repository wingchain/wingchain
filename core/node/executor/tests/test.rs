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

use chrono::Local;
use crypto::hash::HashImpl;
use module_system::InitParams;
use node_db::{DBKey, DB};
use node_executor::{module, Context, Executor, ModuleEnum};
use node_statedb::{StateDB, TrieRoot};
use parity_codec::alloc::collections::HashMap;
use parity_codec::{Decode, Encode};
use primitives::{Transaction, Hash};
use std::sync::Arc;

#[test]
fn test_executor() {
	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = Arc::new(DB::open(&path).unwrap());
	let hasher = Arc::new(HashImpl::Blake2b160);

	let meta_statedb =
		Arc::new(StateDB::new(db.clone(), node_db::columns::META_STATE, hasher.clone()).unwrap());
	let payload_statedb = Arc::new(
		StateDB::new(db.clone(), node_db::columns::PAYLOAD_STATE, hasher.clone()).unwrap(),
	);

	let trie_root = Arc::new(TrieRoot::new(hasher.clone()).unwrap());

	let timestamp = Local::now().timestamp() as u32;

	let executor = Executor::new();

	// block 0
	let txs_0 = vec![
		Arc::new(
			executor
				.build_tx(
					ModuleEnum::System,
					module::system::MethodEnum::Init,
					module::system::InitParams {
						chain_id: "chain-001".to_string(),
						timestamp,
					},
				)
				.unwrap(),
		),
		Arc::new(
			executor
				.build_tx(
					ModuleEnum::System,
					module::system::MethodEnum::Init,
					module::system::InitParams {
						chain_id: "chain-002".to_string(),
						timestamp: timestamp + 1,
					},
				)
				.unwrap(),
		),
	];

	let number = 0;

	let meta_state_root = meta_statedb.default_root();
	let payload_state_root = meta_statedb.default_root();

	let context = Context::new(
		number,
		timestamp,
		trie_root.clone(),
		meta_statedb.clone(),
		Hash(meta_state_root),
		payload_statedb.clone(),
		Hash(payload_state_root),
	)
		.unwrap();

	executor.execute_txs(&context, txs_0.clone()).unwrap();
	let (state_root, transaction) = context.get_meta_update().unwrap();

	let (txs_root, _txs) = context.get_meta_txs().unwrap();

	assert_eq!(txs_root, expected_txs_root(txs_0.clone()));
	assert_eq!(state_root, expected_state_root_0(txs_0.clone()));

	// commit
	db.write(transaction).unwrap();

	// block 1
	let txs_1 = vec![
		Arc::new(
			executor
				.build_tx(
					ModuleEnum::System,
					module::system::MethodEnum::Init,
					module::system::InitParams {
						chain_id: "chain-003".to_string(),
						timestamp: timestamp + 2,
					},
				)
				.unwrap(),
		),
		Arc::new(
			executor
				.build_tx(
					ModuleEnum::System,
					module::system::MethodEnum::Init,
					module::system::InitParams {
						chain_id: "chain-004".to_string(),
						timestamp: timestamp + 3,
					},
				)
				.unwrap(),
		),
	];

	let number = 1;

	let meta_state_root = state_root;
	let payload_state_root = meta_statedb.default_root();

	let context = Context::new(
		number,
		timestamp,
		trie_root.clone(),
		meta_statedb,
		meta_state_root,
		payload_statedb,
		Hash(payload_state_root),
	)
		.unwrap();

	executor.execute_txs(&context, txs_1.clone()).unwrap();
	let (state_root, _) = context.get_meta_update().unwrap();
	let (txs_root, _txs) = context.get_meta_txs().unwrap();

	assert_eq!(txs_root, expected_txs_root(txs_1.clone()));
	assert_eq!(
		state_root,
		expected_state_root_1(txs_0.clone(), txs_1.clone())
	);
}

fn expected_txs_root(txs: Vec<Arc<Transaction>>) -> Hash {
	let trie_root = TrieRoot::new(Arc::new(HashImpl::Blake2b160)).unwrap();
	let txs = txs.into_iter().map(|x| Encode::encode(&x));
	Hash(trie_root.calc_ordered_trie_root(txs))
}

fn expected_state_root_0(txs: Vec<Arc<Transaction>>) -> Hash {
	let tx = &txs[1]; // use the last tx
	let params: InitParams = Decode::decode(&mut &tx.call.params.0[..]).unwrap();

	let data = vec![
		(
			DBKey::from_slice(b"system_chain_id"),
			Some(params.chain_id.encode()),
		),
		(
			DBKey::from_slice(b"system_timestamp"),
			Some(params.timestamp.encode()),
		),
	]
		.into_iter()
		.collect::<HashMap<_, _>>();

	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = Arc::new(DB::open(&path).unwrap());
	let hasher = Arc::new(HashImpl::Blake2b160);

	let statedb =
		Arc::new(StateDB::new(db.clone(), node_db::columns::META_STATE, hasher.clone()).unwrap());

	let (state_root, _) = statedb
		.prepare_update(&statedb.default_root(), data.iter())
		.unwrap();
	Hash(state_root)
}

fn expected_state_root_1(txs_0: Vec<Arc<Transaction>>, txs_1: Vec<Arc<Transaction>>) -> Hash {
	let tx = &txs_0[1]; // use the last tx
	let params: InitParams = Decode::decode(&mut &tx.call.params.0[..]).unwrap();

	let data = vec![
		(
			DBKey::from_slice(b"system_chain_id"),
			Some(params.chain_id.encode()),
		),
		(
			DBKey::from_slice(b"system_timestamp"),
			Some(params.timestamp.encode()),
		),
	]
		.into_iter()
		.collect::<HashMap<_, _>>();

	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = Arc::new(DB::open(&path).unwrap());
	let hasher = Arc::new(HashImpl::Blake2b160);

	let statedb =
		Arc::new(StateDB::new(db.clone(), node_db::columns::META_STATE, hasher.clone()).unwrap());

	let (state_root, transcation) = statedb
		.prepare_update(&statedb.default_root(), data.iter())
		.unwrap();

	db.write(transcation).unwrap();

	let tx = &txs_1[1]; // use the last tx
	let params: InitParams = Decode::decode(&mut &tx.call.params.0[..]).unwrap();

	let data = vec![
		(
			DBKey::from_slice(b"system_chain_id"),
			Some(params.chain_id.encode()),
		),
		(
			DBKey::from_slice(b"system_timestamp"),
			Some(params.timestamp.encode()),
		),
	]
		.into_iter()
		.collect::<HashMap<_, _>>();

	let (state_root, _) = statedb.prepare_update(&state_root, data.iter()).unwrap();
	Hash(state_root)
}
