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
use std::str::FromStr;
use std::sync::Arc;

use hash_db::Hasher;
use mut_static::MutStatic;

use crypto::hash::Hash;
use crypto::hash::HashImpl;
use lazy_static::lazy_static;
use node_db::{DBConfig, DB};
use node_statedb::{StateDB, TrieRoot};
use primitives::{codec, DBKey};

#[test]
fn test_static_hash() {
	lazy_static! {
		pub static ref HASH_IMPL_1: MutStatic<Arc<HashImpl>> = MutStatic::new();
	}

	let hash = Arc::new(HashImpl::from_str("blake2b_256").unwrap());

	HASH_IMPL_1.set(hash.clone()).unwrap();

	let name = HASH_IMPL_1.read().unwrap().name();
	assert_eq!("blake2b_256", name);
}

#[test]
fn test_wrapped_hash() {
	lazy_static! {
		pub static ref HASH_IMPL_2: MutStatic<Arc<HashImpl>> = MutStatic::new();
	}

	struct DynamicHasher256;

	#[derive(Default)]
	struct DummyStdHasher;

	impl std::hash::Hasher for DummyStdHasher {
		fn finish(&self) -> u64 {
			0
		}
		fn write(&mut self, _bytes: &[u8]) {}
	}

	impl Hasher for DynamicHasher256 {
		type Out = [u8; 32];
		type StdHasher = DummyStdHasher;
		const LENGTH: usize = 32;

		/// Compute the hash of the provided slice of bytes returning the `Out` type of the `Hasher`.
		fn hash(x: &[u8]) -> Self::Out {
			let hasher = HASH_IMPL_2.read().unwrap();
			let mut out = [0u8; 32];
			hasher.hash(&mut out, x);
			out
		}
	}

	let hash = Arc::new(HashImpl::from_str("blake2b_256").expect(""));

	HASH_IMPL_2.set(hash.clone()).unwrap();

	let data = [1u8, 2u8, 3u8];
	let out = DynamicHasher256::hash(&data);

	assert_eq!(
		out,
		[
			17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142, 220,
			157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
		]
	);
}

#[test]
fn test_statedb_256() {
	test_statedb_for_hasher(HashImpl::Blake2b256);
}

#[test]
fn test_statedb_512() {
	test_statedb_for_hasher(HashImpl::Blake2b512);
}

#[test]
fn test_statedb_256_reopen() {
	use tempfile::tempdir;

	let path = tempdir().expect("Could not create a temp dir");
	let path = path.into_path();

	let db_config = DBConfig {
		memory_budget: 1 * 1024 * 1024,
		path,
		partitions: vec![],
	};

	let db = Arc::new(DB::open(db_config.clone()).unwrap());

	let hasher = HashImpl::Blake2b512;

	let hasher = Arc::new(hasher);

	let hasher_clone = hasher.clone();

	let hasher_len: usize = hasher.length().into();

	let statedb = StateDB::new(db.clone(), node_db::columns::PAYLOAD_STATE, hasher_clone).unwrap();

	let root = statedb.default_root();

	assert_eq!(root.len(), hasher_len);

	// update 1
	let data = vec![(DBKey::from_slice(b"abc"), Some(vec![1u8; 1024]))]
		.into_iter()
		.collect::<HashMap<_, _>>();
	let (update_1_root, transaction) = statedb.prepare_update(&root, data.iter()).unwrap();
	db.write(transaction).unwrap();
	let result = statedb.get(&update_1_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![1u8; 1024]), result);

	// update 2
	let data = vec![(DBKey::from_slice(b"abc"), Some(vec![2u8; 1024]))]
		.into_iter()
		.collect::<HashMap<_, _>>();
	let (update_2_root, transaction) = statedb.prepare_update(&update_1_root, data.iter()).unwrap();
	db.write(transaction).unwrap();
	let result = statedb.get(&update_2_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![2u8; 1024]), result);

	let result = statedb.get(&update_1_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![1u8; 1024]), result);

	drop(statedb);
	drop(db);

	let hasher_clone = hasher.clone();
	let db = Arc::new(DB::open(db_config).unwrap());
	let statedb = StateDB::new(db, node_db::columns::PAYLOAD_STATE, hasher_clone).unwrap();

	let result = statedb.get(&update_2_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![2u8; 1024]), result);

	let result = statedb.get(&update_1_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![1u8; 1024]), result);
}

fn test_statedb_for_hasher(hasher: HashImpl) {
	use tempfile::tempdir;

	let path = tempdir().expect("Could not create a temp dir");
	let path = path.into_path();

	let db_config = DBConfig {
		memory_budget: 1 * 1024 * 1024,
		path,
		partitions: vec![],
	};

	let db = Arc::new(DB::open(db_config).unwrap());

	let hasher = Arc::new(hasher);

	let hasher_len: usize = hasher.length().into();

	let statedb = StateDB::new(db.clone(), node_db::columns::PAYLOAD_STATE, hasher).unwrap();

	let root = statedb.default_root();

	assert_eq!(root.len(), hasher_len);

	// update 1
	let data = vec![
		(DBKey::from_slice(b"abc"), Some(vec![1u8; 1024])),
		(DBKey::from_slice(b"abd"), Some(vec![1u8; 1024])),
	]
	.into_iter()
	.collect::<HashMap<_, _>>();

	let (update_1_root, transaction) = statedb.prepare_update(&root, data.iter()).unwrap();
	db.write(transaction).unwrap();
	let result = statedb.get(&update_1_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![1u8; 1024]), result);

	// update 2
	let data = vec![(DBKey::from_slice(b"abc"), Some(vec![2u8; 1024]))]
		.into_iter()
		.collect::<HashMap<_, _>>();
	let (update_2_root, transaction) = statedb.prepare_update(&update_1_root, data.iter()).unwrap();
	db.write(transaction).unwrap();
	let result = statedb.get(&update_2_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![2u8; 1024]), result);

	let result = statedb.get(&update_1_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![1u8; 1024]), result);

	// update 3
	let data = vec![(DBKey::from_slice(b"abc"), None)]
		.into_iter()
		.collect::<HashMap<_, _>>();
	let (update_3_root, transaction) = statedb.prepare_update(&update_2_root, data.iter()).unwrap();
	db.write(transaction).unwrap();
	let result = statedb.get(&update_3_root, &b"abc"[..]).unwrap();

	assert_eq!(None, result);

	let result = statedb.get(&update_1_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![1u8; 1024]), result);

	let result = statedb.get(&update_2_root, &b"abc"[..]).unwrap();

	assert_eq!(Some(vec![2u8; 1024]), result);

	// use getter
	let stmt = statedb.prepare_stmt(&update_2_root).unwrap();
	let getter = StateDB::prepare_get(&stmt).unwrap();
	let result = getter.get(&b"abc"[..]).unwrap();
	assert_eq!(Some(vec![2u8; 1024]), result);

	let result = getter.get(&b"abd"[..]).unwrap();
	assert_eq!(Some(vec![1u8; 1024]), result);

	let result = getter.get(&b"abd"[..]).unwrap();
	assert_eq!(Some(vec![1u8; 1024]), result);
}

#[test]
fn test_calc_trie_root() {
	let hasher = Arc::new(HashImpl::Blake2b512);

	let trie_root = TrieRoot::new(hasher).unwrap();

	let input = vec!["a", "b"];

	let root = trie_root.calc_trie_root(
		input
			.iter()
			.enumerate()
			.map(|(k, v)| (codec::encode(&(k as u32)).unwrap(), v))
			.collect::<Vec<_>>(),
	);

	assert_eq!(
		root,
		vec![
			39, 107, 232, 90, 112, 186, 33, 13, 177, 69, 54, 175, 214, 152, 221, 220, 241, 202, 83,
			94, 135, 219, 5, 8, 85, 48, 154, 106, 51, 145, 113, 54, 112, 199, 82, 147, 147, 239,
			243, 36, 165, 104, 233, 123, 2, 95, 250, 7, 87, 94, 129, 44, 102, 95, 118, 159, 131,
			61, 53, 34, 181, 186, 54, 214
		]
	);

	let root = trie_root.calc_ordered_trie_root(&input);

	assert_eq!(
		root,
		vec![
			39, 107, 232, 90, 112, 186, 33, 13, 177, 69, 54, 175, 214, 152, 221, 220, 241, 202, 83,
			94, 135, 219, 5, 8, 85, 48, 154, 106, 51, 145, 113, 54, 112, 199, 82, 147, 147, 239,
			243, 36, 165, 104, 233, 123, 2, 95, 250, 7, 87, 94, 129, 44, 102, 95, 118, 159, 131,
			61, 53, 34, 181, 186, 54, 214
		]
	);
}

#[cfg(feature = "build-dep-test")]
mod build_dep_test {
	use super::*;

	#[test]
	fn test_statedb_160_dylib() {
		let path = utils_test::get_dylib("crypto_dylib_samples_hash");

		assert!(
			path.exists(),
			"Should build first to make exist: {:?}",
			path
		);

		let path = path.to_string_lossy();
		let hasher = HashImpl::from_str(&path).unwrap();

		let name = hasher.name();
		assert_eq!(name, "blake2b_160".to_string());

		test_statedb_for_hasher(hasher);
	}
}
