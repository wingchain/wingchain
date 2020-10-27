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

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::HashImpl;
use node_db::DB;
use node_executor::{module, Context, ContextEssence, Executor};
use node_executor_primitives::ContextEnv;
use node_statedb::{StateDB, TrieRoot};
use primitives::types::FullReceipt;
use primitives::{
	codec, Address, Balance, DBKey, Event, FullTransaction, Hash, Params, Receipt,
	TransactionForHash,
};
use utils_test::test_accounts;

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

	let timestamp = 1588146696502;

	let (account1, account2) = test_accounts(dsa.clone(), address.clone());

	let executor = Executor::new(hasher, dsa, address);

	// block 0
	let block_0_meta_txs = vec![executor
		.build_tx(
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
	let block_0_payload_txs = vec![executor
		.build_tx(
			None,
			"balance".to_string(),
			"init".to_string(),
			module::balance::InitParams {
				endow: vec![(account1.3.clone(), 10)],
			},
		)
		.unwrap()];

	let block_0_meta_txs = block_0_meta_txs
		.into_iter()
		.map(|tx| {
			let tx_hash = executor.hash_transaction(&tx).unwrap();
			Arc::new(FullTransaction { tx, tx_hash })
		})
		.collect::<Vec<_>>();
	let block_0_payload_txs = block_0_payload_txs
		.into_iter()
		.map(|tx| {
			let tx_hash = executor.hash_transaction(&tx).unwrap();
			Arc::new(FullTransaction { tx, tx_hash })
		})
		.collect::<Vec<_>>();

	let number = 0;

	let meta_state_root = meta_statedb.default_root();
	let payload_state_root = meta_statedb.default_root();

	let env = ContextEnv { number, timestamp };

	let context_essence = ContextEssence::new(
		env,
		trie_root.clone(),
		meta_statedb.clone(),
		Hash(meta_state_root),
		payload_statedb.clone(),
		Hash(payload_state_root),
	)
	.unwrap();
	let context = Context::new(&context_essence).unwrap();

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

	let (meta_receipts_root, meta_receipts) = context.get_meta_receipts().unwrap();
	let (payload_receipts_root, payload_receipts) = context.get_payload_receipts().unwrap();

	assert_eq!(meta_txs_root, expected_txs_root(&block_0_meta_txs));
	assert_eq!(payload_txs_root, expected_txs_root(&block_0_payload_txs));
	assert_eq!(
		meta_state_root,
		expected_block_0_meta_state_root(&block_0_meta_txs)
	);
	assert_eq!(
		payload_state_root,
		expected_block_0_payload_state_root(&block_0_payload_txs)
	);
	assert_eq!(
		(meta_receipts_root, meta_receipts),
		expected_block_0_receipts_root(&block_0_meta_txs)
	);
	assert_eq!(
		(payload_receipts_root, payload_receipts),
		expected_block_0_receipts_root(&block_0_payload_txs)
	);

	// commit
	db.write(meta_transaction).unwrap();
	db.write(payload_transaction).unwrap();

	// block 1

	let nonce = 0u32;
	let until = 1u64;

	let tx = executor
		.build_tx(
			Some((account1.0.clone(), nonce, until)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();
	executor.validate_tx(&tx, true).unwrap();

	let block_1_meta_txs = vec![];

	let block_1_payload_txs = vec![tx]
		.into_iter()
		.map(|tx| {
			let tx_hash = executor.hash_transaction(&tx).unwrap();
			Arc::new(FullTransaction { tx, tx_hash })
		})
		.collect::<Vec<_>>();

	let number = 1;
	let timestamp = timestamp + 1;

	let env = ContextEnv { number, timestamp };

	let context_essence = ContextEssence::new(
		env,
		trie_root.clone(),
		meta_statedb.clone(),
		meta_state_root,
		payload_statedb.clone(),
		payload_state_root,
	)
	.unwrap();
	let context = Context::new(&context_essence).unwrap();

	executor
		.execute_txs(&context, block_1_meta_txs.clone())
		.unwrap();
	executor
		.execute_txs(&context, block_1_payload_txs.clone())
		.unwrap();
	let (meta_state_root, _meta_transaction) = context.get_meta_update().unwrap();
	let (payload_state_root, _payload_transaction) = context.get_payload_update().unwrap();

	let (meta_txs_root, _txs) = context.get_meta_txs().unwrap();
	let (payload_txs_root, _txs) = context.get_payload_txs().unwrap();
	let (payload_receipts_root, payload_receipts) = context.get_payload_receipts().unwrap();

	assert_eq!(meta_txs_root, expected_txs_root(&block_1_meta_txs));
	assert_eq!(payload_txs_root, expected_txs_root(&block_1_payload_txs));
	assert_eq!(
		meta_state_root,
		expected_block_0_meta_state_root(&block_0_meta_txs)
	);
	assert_eq!(
		payload_state_root,
		expected_block_1_payload_state_root(&block_0_payload_txs, &block_1_payload_txs)
	);
	assert_eq!(
		(payload_receipts_root, payload_receipts),
		expected_block_1_receipts_root(&block_1_payload_txs, account1.3, account2.3, 2)
	);
}

#[test]
fn test_executor_validate_tx() {
	let hasher = Arc::new(HashImpl::Blake2b256);
	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, account2) = test_accounts(dsa.clone(), address.clone());

	let executor = Executor::new(hasher, dsa, address);

	let nonce = 0u32;
	let until = 1u64;

	// invalid tx
	let tx = executor.build_tx(
		Some((account1.0.clone(), nonce, until)),
		"balance".to_string(),
		"transfer".to_string(),
		module::balance::TransferParams {
			recipient: Address(vec![1u8]),
			value: 2,
		},
	);
	assert!(format!("{}", tx.unwrap_err()).contains("Error: Invalid address"));

	let tx = executor.build_tx(
		Some((account1.0.clone(), nonce, until)),
		"unknown".to_string(),
		"transfer".to_string(),
		module::balance::TransferParams {
			recipient: Address(vec![1u8]),
			value: 2,
		},
	);
	assert!(format!("{}", tx.unwrap_err()).contains("Error: Invalid tx module"));

	let tx = executor.build_tx(
		Some((account1.0.clone(), nonce, until)),
		"balance".to_string(),
		"unknown".to_string(),
		module::balance::TransferParams {
			recipient: Address(vec![1u8]),
			value: 2,
		},
	);
	assert!(format!("{}", tx.unwrap_err()).contains("Error: Invalid tx method"));

	let tx = executor.build_tx(
		Some((account1.0.clone(), nonce, until)),
		"balance".to_string(),
		"transfer".to_string(),
		Params(vec![0u8]),
	);
	assert!(format!("{}", tx.unwrap_err()).contains("Error: Invalid tx params"));

	let tx = {
		let mut tx = executor
			.build_tx(
				Some((account1.0.clone(), nonce, until)),
				"balance".to_string(),
				"transfer".to_string(),
				module::balance::TransferParams {
					recipient: account2.3.clone(),
					value: 2,
				},
			)
			.unwrap();
		let mut witness = tx.witness.unwrap().clone();
		witness.public_key = account2.1;
		tx.witness = Some(witness);
		tx
	};
	let result = executor.validate_tx(&tx, true);
	assert!(format!("{}", result.unwrap_err()).contains("Error: Invalid tx witness"));
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

fn expected_block_1_payload_state_root(
	block_0_txs: &Vec<Arc<FullTransaction>>,
	block_1_txs: &Vec<Arc<FullTransaction>>,
) -> Hash {
	let tx = &block_0_txs[0].tx; // use the last tx
	let params: module::balance::InitParams = codec::decode(&tx.call.params.0[..]).unwrap();
	let (account1, balance) = &params.endow[0];

	let data = vec![(
		DBKey::from_slice(
			&[
				&b"balance_balance_"[..],
				&codec::encode(&account1.0).unwrap(),
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

	let (state_root, transaction) = statedb
		.prepare_update(&statedb.default_root(), data.iter())
		.unwrap();

	db.write(transaction).unwrap();

	let tx = &block_1_txs[0].tx; // use the last tx
	let params: module::balance::TransferParams = codec::decode(&tx.call.params.0[..]).unwrap();

	let (account2, value) = (&params.recipient, params.value);

	let data = vec![
		(
			DBKey::from_slice(
				&[
					&b"balance_balance_"[..],
					&codec::encode(&account1.0).unwrap(),
				]
				.concat(),
			),
			Some(codec::encode(&(balance - value)).unwrap()),
		),
		(
			DBKey::from_slice(
				&[
					&b"balance_balance_"[..],
					&codec::encode(&account2.0).unwrap(),
				]
				.concat(),
			),
			Some(codec::encode(&value).unwrap()),
		),
	]
	.into_iter()
	.collect::<HashMap<_, _>>();

	let (state_root, _) = statedb.prepare_update(&state_root, data.iter()).unwrap();
	Hash(state_root)
}

fn expected_block_1_receipts_root(
	txs: &Vec<Arc<FullTransaction>>,
	sender: Address,
	recipient: Address,
	value: Balance,
) -> (Hash, Vec<Arc<FullReceipt>>) {
	let trie_root = TrieRoot::new(Arc::new(HashImpl::Blake2b256)).unwrap();

	let event = module::balance::TransferEvent::Transferred(module::balance::Transferred {
		sender,
		recipient,
		value,
	});
	let event = Event::from(&event).unwrap();

	let receipts = txs
		.into_iter()
		.map(|x| {
			Arc::new(FullReceipt {
				tx_hash: x.tx_hash.clone(),
				receipt: Receipt {
					block_number: 1,
					events: vec![event.clone()],
					result: Ok(codec::encode(&()).unwrap()),
				},
			})
		})
		.collect::<Vec<_>>();

	let map = receipts.iter().map(|x| codec::encode(&x.receipt).unwrap());
	let root = Hash(trie_root.calc_ordered_trie_root(map));

	(root, receipts)
}
