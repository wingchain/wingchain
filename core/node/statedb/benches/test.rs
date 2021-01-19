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

#![feature(test)]

extern crate test;

use std::collections::HashMap;
use std::sync::Arc;
use test::{black_box, Bencher};

use crypto::hash::HashImpl;
use node_db::{DBConfig, DB};
use node_statedb::StateDB;
use primitives::DBKey;

#[bench]
fn bench_statedb_get_1(b: &mut Bencher) {
	let (statedb, root) = prepare_statedb(1);

	b.iter(|| black_box(statedb.get(&root, &b"abc"[..])));
}

#[bench]
fn bench_statedb_get_with_getter_1(b: &mut Bencher) {
	let (statedb, root) = prepare_statedb(1);

	let stmt = statedb.prepare_stmt(&root).unwrap();

	let getter = StateDB::prepare_get(&stmt).unwrap();

	b.iter(|| black_box(getter.get(&&b"abc"[..])));
}

#[bench]
fn bench_statedb_get_2(b: &mut Bencher) {
	let (statedb, root) = prepare_statedb(2);

	b.iter(|| black_box(statedb.get(&root, &b"abc"[..])));
}

#[bench]
fn bench_statedb_get_with_getter_2(b: &mut Bencher) {
	let (statedb, root) = prepare_statedb(2);

	let stmt = statedb.prepare_stmt(&root).unwrap();

	let getter = StateDB::prepare_get(&stmt).unwrap();

	b.iter(|| black_box(getter.get(&&b"abc"[..])));
}

fn prepare_statedb(records: usize) -> (StateDB, Vec<u8>) {
	use tempfile::tempdir;

	let path = tempdir().expect("Could not create a temp dir");
	let path = path.into_path();

	let db_config = DBConfig {
		memory_budget: 128 * 1024 * 1024,
		path,
		partitions: vec![],
	};

	let db = Arc::new(DB::open(db_config).unwrap());

	let hasher = Arc::new(HashImpl::Blake2b160);

	let statedb = StateDB::new(db.clone(), node_db::columns::META_STATE, hasher).unwrap();

	let root = statedb.default_root();

	// update 1
	let data = match records {
		1 => vec![(DBKey::from_slice(b"abc"), Some(vec![1u8; 1024]))],
		2 => vec![
			(DBKey::from_slice(b"abc"), Some(vec![1u8; 1024])),
			(DBKey::from_slice(b"abd"), Some(vec![1u8; 1024])),
		],
		_ => unreachable!(),
	}
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

	(statedb, update_2_root)
}
