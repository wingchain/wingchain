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

use tempfile::tempdir;

use node_db::columns;
use node_db::global_key;
use node_db::{DBKey, DB};

#[test]
fn test_db() {
	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = DB::open(&path).unwrap();

	let mut transaction = db.transaction();
	transaction.put(columns::GLOBAL, &global_key::BEST_NUMBER, &vec![1, 0, 0, 0]);
	transaction.put(columns::BLOCK_HASH, &vec![1, 0, 0, 0], b"header");
	transaction.put(columns::HEADER, b"header", b"header_value");
	transaction.put_owned(
		columns::PAYLOAD_TXS,
		DBKey::from_vec(b"body".to_vec()),
		b"body_value".to_vec(),
	);

	db.write(transaction).unwrap();

	assert_eq!(
		db.get(columns::GLOBAL, &global_key::BEST_NUMBER)
			.unwrap()
			.unwrap(),
		vec![1u8, 0u8, 0u8, 0u8]
	);
	assert_eq!(
		db.get(columns::BLOCK_HASH, &vec![1, 0, 0, 0])
			.unwrap()
			.unwrap(),
		b"header"
	);
	assert_eq!(
		db.get(columns::HEADER, b"header").unwrap().unwrap(),
		b"header_value"
	);
	assert_eq!(
		db.get(columns::PAYLOAD_TXS, b"body").unwrap().unwrap(),
		b"body_value"
	);
}

#[test]
fn test_existing_db() {
	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = DB::open(&path).unwrap();

	let mut transaction = db.transaction();
	transaction.put(columns::GLOBAL, &global_key::BEST_NUMBER, &vec![1, 0, 0, 0]);

	db.write(transaction).unwrap();

	drop(db);

	let db = DB::open(&path).unwrap();
	assert_eq!(
		db.get(columns::GLOBAL, &global_key::BEST_NUMBER)
			.unwrap()
			.unwrap(),
		vec![1u8, 0u8, 0u8, 0u8]
	);
}

#[test]
fn test_db_concurrent() {
	use std::sync::Arc;
	use std::thread;

	let path = tempdir().expect("could not create a temp dir");
	let path = path.into_path();

	let db = Arc::new(DB::open(&path).unwrap());

	let db1 = db.clone();
	let db2 = db.clone();

	let thread1 = thread::spawn(move || {
		let mut transaction = db1.transaction();
		transaction.put(columns::GLOBAL, &global_key::BEST_NUMBER, &vec![1, 0, 0, 0]);
		db1.write(transaction).unwrap();
	});

	let thread2 = thread::spawn(move || {
		let mut transaction = db2.transaction();
		transaction.put(columns::GLOBAL, &global_key::BEST_NUMBER, &vec![2, 0, 0, 0]);
		db2.write(transaction).unwrap();
	});

	thread1.join().unwrap();
	thread2.join().unwrap();

	let result = db
		.get(columns::GLOBAL, &global_key::BEST_NUMBER)
		.unwrap()
		.unwrap();

	assert!(result == vec![1, 0, 0, 0] || result == vec![2, 0, 0, 0])
}
