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
use std::sync::Arc;

use chrono::Local;

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{Dsa, DsaImpl, KeyPair};
use crypto::hash::HashImpl;
use node_db::DB;
use node_executor::{module, Context, Executor};
use node_executor_primitives::ContextEnv;
use node_statedb::{StateDB, TrieRoot};
use primitives::{codec, Address, DBKey, Hash, PublicKey, Signature, Transaction, Witness};
use std::rc::Rc;

#[test]
fn test_executor() {
	use tempfile::tempdir;

	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = Arc::new(DB::open(&path).unwrap());
	let hasher = Arc::new(HashImpl::Blake2b256);
	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let meta_statedb =
		Arc::new(StateDB::new(db.clone(), node_db::columns::META_STATE, hasher.clone()).unwrap());
	let payload_statedb = Arc::new(
		StateDB::new(db.clone(), node_db::columns::PAYLOAD_STATE, hasher.clone()).unwrap(),
	);

	let trie_root = Arc::new(TrieRoot::new(hasher.clone()).unwrap());

	let timestamp = Local::now().timestamp() as u32;

	let (secret_key_len, public_key_len, sig_len) = dsa.length().into();
	let address_len = address.length().into();

	let account1 = {
		let secret_key = vec![1u8; secret_key_len];

		let key_pair = dsa.key_pair_from_secret_key(&secret_key).unwrap();
		let public_key = PublicKey({
			let mut out = vec![0u8; public_key_len];
			key_pair.public_key(&mut out);
			out
		});
		let account = Address({
			let mut out = vec![0u8; address_len];
			address.address(&mut out, &public_key.0);
			out
		});
		(secret_key, public_key, key_pair, account)
	};

	let account2 = {
		let secret_key = vec![2u8; secret_key_len];

		let key_pair = dsa.key_pair_from_secret_key(&secret_key).unwrap();
		let public_key = PublicKey({
			let mut out = vec![0u8; public_key_len];
			key_pair.public_key(&mut out);
			out
		});
		let account = Address({
			let mut out = vec![0u8; address_len];
			address.address(&mut out, &public_key.0);
			out
		});
		(secret_key, public_key, key_pair, account)
	};

	let executor = Executor::new(dsa, address);

	// block 0
	let block_0_meta_txs = vec![Arc::new(
		executor
			.build_tx(
				"system".to_string(),
				"init".to_string(),
				module::system::InitParams {
					chain_id: "chain-test".to_string(),
					timestamp,
				},
			)
			.unwrap(),
	)];
	let block_0_payload_txs = vec![Arc::new(
		executor
			.build_tx(
				"balance".to_string(),
				"init".to_string(),
				module::balance::InitParams {
					endow: vec![(account1.3, 10)],
				},
			)
			.unwrap(),
	)];

	let number = 0;

	let meta_state_root = meta_statedb.default_root();
	let payload_state_root = meta_statedb.default_root();

	let env = Rc::new(ContextEnv { number, timestamp });

	let context = Context::new(
		env,
		trie_root.clone(),
		meta_statedb.clone(),
		Hash(meta_state_root),
		payload_statedb.clone(),
		Hash(payload_state_root),
	)
	.unwrap();

	executor
		.execute_txs(&context, block_0_meta_txs.clone())
		.unwrap();
	executor
		.execute_txs(&context, block_0_payload_txs.clone())
		.unwrap();
	let (meta_state_root, meta_transaction) = context.get_meta_update().unwrap();
	let (payload_state_root, payload_transaction) = context.get_payload_update().unwrap();

	let (meta_txs_root, _txs) = context.get_meta_txs().unwrap();
	let (payload_txs_root, _txs) = context.get_payload_txs().unwrap();

	assert_eq!(meta_txs_root, expected_txs_root(block_0_meta_txs.clone()));
	assert_eq!(
		payload_txs_root,
		expected_txs_root(block_0_payload_txs.clone())
	);
	assert_eq!(
		meta_state_root,
		expected_block_0_meta_state_root(block_0_meta_txs.clone())
	);
	assert_eq!(
		payload_state_root,
		expected_block_0_payload_state_root(block_0_payload_txs.clone())
	);

	// commit
	db.write(meta_transaction).unwrap();
	db.write(payload_transaction).unwrap();

	// block 1

	let nonce = 0u32;
	let until = 1u32;
	let mut tx = executor
		.build_tx(
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3,
				value: 2,
			},
		)
		.unwrap();
	let message = codec::encode(&(nonce, until, &tx.call)).unwrap();
	let sig = Signature({
		let mut out = vec![0u8; sig_len];
		account1.2.sign(&message, &mut out);
		out
	});
	tx.witness = Some(Witness {
		public_key: account1.1,
		signature: sig,
		nonce,
		until,
	});
	executor.validate_tx(&tx).unwrap();

	let block_1_meta_txs = vec![];

	let block_1_payload_txs = vec![Arc::new(tx)];

	let number = 1;
	let timestamp = timestamp + 1;

	let env = Rc::new(ContextEnv { number, timestamp });

	let context = Context::new(
		env,
		trie_root.clone(),
		meta_statedb,
		meta_state_root,
		payload_statedb,
		payload_state_root,
	)
	.unwrap();

	executor
		.execute_txs(&context, block_1_meta_txs.clone())
		.unwrap();
	// executor.execute_txs(&context, block_1_payload_txs.clone()).unwrap();
	// let (meta_state_root, meta_transaction) = context.get_meta_update().unwrap();
	// let (payload_state_root, payload_transaction) = context.get_payload_update().unwrap();

	// let (meta_txs_root, _txs) = context.get_meta_txs().unwrap();
	// let (payload_txs_root, _txs) = context.get_payload_txs().unwrap();
	//
	// assert_eq!(txs_root, expected_txs_root(txs_1.clone()));
	// assert_eq!(
	// 	state_root,
	// 	expected_state_root_1(txs_0.clone(), txs_1.clone())
	// );
}

fn expected_txs_root(txs: Vec<Arc<Transaction>>) -> Hash {
	let trie_root = TrieRoot::new(Arc::new(HashImpl::Blake2b256)).unwrap();
	let txs = txs.into_iter().map(|x| codec::encode(&*x).unwrap());
	Hash(trie_root.calc_ordered_trie_root(txs))
}

fn expected_block_0_meta_state_root(txs: Vec<Arc<Transaction>>) -> Hash {
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

fn expected_block_0_payload_state_root(txs: Vec<Arc<Transaction>>) -> Hash {
	let tx = &txs[0]; // use the last tx
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

fn expected_state_root_1(txs_0: Vec<Arc<Transaction>>, txs_1: Vec<Arc<Transaction>>) -> Hash {
	let tx = &txs_0[0]; // use the last tx
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

	let (state_root, transaction) = statedb
		.prepare_update(&statedb.default_root(), data.iter())
		.unwrap();

	db.write(transaction).unwrap();

	let tx = &txs_1[0]; // use the last tx
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

	let (state_root, _) = statedb.prepare_update(&state_root, data.iter()).unwrap();
	Hash(state_root)
}
