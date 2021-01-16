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

//! Key value db based on rocksdb

use std::path::Path;

use parking_lot::RwLock;
use rocksdb::{
	BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, Options, ReadOptions, WriteBatch,
	WriteOptions, DB as RocksDB,
};

use primitives::errors::CommonResult;
use primitives::{DBKey, DBValue};

use crate::config::{gen_block_opts, gen_cf_opts, gen_db_opts, gen_read_opts, gen_write_opts};

pub mod config;
pub mod errors;

pub struct DB {
	db: RwLock<RocksDB>,
	#[allow(dead_code)]
	db_opts: Options,
	write_opts: WriteOptions,
	read_opts: ReadOptions,
	#[allow(dead_code)]
	block_opts: BlockBasedOptions,
}

impl DB {
	/// Open the db from the given path
	pub fn open(path: &Path) -> CommonResult<DB> {
		let col_count = columns::COLUMN_NAMES.len();
		let db_opts = gen_db_opts(col_count);
		let block_opts = gen_block_opts();
		let read_opts = gen_read_opts();
		let write_opts = gen_write_opts();

		let cfs = columns::COLUMN_NAMES
			.iter()
			.map(|&name| ColumnFamilyDescriptor::new(name, gen_cf_opts(col_count, &block_opts)));

		let rocksdb = match RocksDB::open_cf_descriptors(&db_opts, path, cfs) {
			Err(_) => match RocksDB::open_cf(&db_opts, path, &[] as &[&str]) {
				Ok(mut db) => {
					for &name in columns::COLUMN_NAMES.iter() {
						let _ = db
							.create_cf(name, &gen_cf_opts(col_count, &block_opts))
							.map_err(errors::ErrorKind::RocksDB)?;
					}
					db
				}
				Err(e) => return Err(errors::ErrorKind::RocksDB(e).into()),
			},
			Ok(db) => db,
		};

		let db = Self {
			db: RwLock::new(rocksdb),
			db_opts,
			write_opts,
			read_opts,
			block_opts,
		};

		Ok(db)
	}

	/// Get value by col and key
	pub fn get(&self, col: u32, key: &[u8]) -> CommonResult<Option<DBValue>> {
		let db = &(*self.db.read());
		let cf = Self::get_cf(&db, col);

		let result = db
			.get_cf_opt(cf, key, &self.read_opts)
			.map_err(errors::ErrorKind::RocksDB)?;

		Ok(result)
	}

	/// Get value processed by f
	pub fn get_with<U, F: FnOnce(DBValue) -> CommonResult<U>>(
		&self,
		col: u32,
		key: &[u8],
		f: F,
	) -> CommonResult<Option<U>> {
		let value = self.get(col, key)?;
		let value = match value {
			Some(value) => Some(f(value)?),
			None => None,
		};
		Ok(value)
	}

	/// Write with a db transaction
	pub fn write(&self, transaction: DBTransaction) -> CommonResult<()> {
		let db = &(*self.db.write());

		let ops = transaction.ops;
		let mut batch = WriteBatch::default();
		for op in ops {
			match op {
				DBOp::Insert { col, key, value } => {
					let cf = Self::get_cf(&db, col);
					batch
						.put_cf(cf, &key, &value)
						.map_err(errors::ErrorKind::RocksDB)?;
				}
				DBOp::Delete { col, key } => {
					let cf = Self::get_cf(&db, col);
					batch
						.delete_cf(cf, &key)
						.map_err(errors::ErrorKind::RocksDB)?;
				}
			};
		}
		db.write_opt(batch, &self.write_opts)
			.map_err(errors::ErrorKind::RocksDB)?;
		Ok(())
	}

	/// Init a new empty db transaction
	pub fn transaction(&self) -> DBTransaction {
		DBTransaction::new()
	}

	fn get_cf(db: &RocksDB, col: u32) -> &ColumnFamily {
		let col_name = columns::COLUMN_NAMES[col as usize];
		let cf = db.cf_handle(col_name).expect("Col name should exist");
		cf
	}
}

#[derive(Debug)]
pub struct DBTransaction {
	ops: Vec<DBOp>,
}

#[derive(Debug)]
pub enum DBOp {
	Insert {
		col: u32,
		key: DBKey,
		value: DBValue,
	},
	Delete {
		col: u32,
		key: DBKey,
	},
}

impl DBTransaction {
	/// Create new transaction.
	pub fn new() -> DBTransaction {
		DBTransaction::with_capacity(256)
	}

	/// Create new transaction with capacity.
	pub fn with_capacity(cap: usize) -> DBTransaction {
		DBTransaction {
			ops: Vec::with_capacity(cap),
		}
	}

	/// Insert a key-value pair in the transaction. Any existing value will be overwritten upon write.
	pub fn put(&mut self, col: u32, key: &[u8], value: &[u8]) {
		self.ops.push(DBOp::Insert {
			col,
			key: DBKey::from_slice(key),
			value: value.to_vec(),
		})
	}

	/// To avoid copy
	pub fn put_owned(&mut self, col: u32, key: DBKey, value: DBValue) {
		self.ops.push(DBOp::Insert { col, key, value });
	}

	/// Delete value by key.
	pub fn delete(&mut self, col: u32, key: &[u8]) {
		self.ops.push(DBOp::Delete {
			col,
			key: DBKey::from_slice(key),
		});
	}

	/// Extend with another transaction
	pub fn extend(&mut self, transaction: DBTransaction) {
		self.ops.extend(transaction.ops);
	}
}

impl Default for DBTransaction {
	fn default() -> Self {
		Self::new()
	}
}

pub mod columns {
	/// column names, which should be corresponding to the following const
	pub const COLUMN_NAMES: [&str; 11] = [
		"global",
		"block_hash",
		"header",
		"meta_state",
		"meta_txs",
		"payload_state",
		"payload_txs",
		"tx",
		"receipt",
		"execution",
		"proof",
	];

	/// see global_key
	pub const GLOBAL: u32 = 0;

	/// block number to block hash
	pub const BLOCK_HASH: u32 = 1;

	/// block hash to header
	pub const HEADER: u32 = 2;

	/// meta state trie
	pub const META_STATE: u32 = 3;

	/// block hash to meta transaction hashes
	pub const META_TXS: u32 = 4;

	/// payload state trie
	pub const PAYLOAD_STATE: u32 = 5;

	/// block hash to payload transaction hashes
	pub const PAYLOAD_TXS: u32 = 6;

	/// transaction hash to transaction
	pub const TX: u32 = 7;

	/// transaction hash to receipt
	pub const RECEIPT: u32 = 8;

	/// block hash to execution
	pub const EXECUTION: u32 = 9;

	/// block hash to proof
	pub const PROOF: u32 = 10;
}

pub mod global_key {
	/// The confirmed number of the chain
	pub const CONFIRMED_NUMBER: &[u8] = b"confirmed_number";
	/// The execution number of the chain
	pub const EXECUTION_NUMBER: &[u8] = b"execution_number";
	/// spec
	pub const SPEC: &[u8] = b"spec";
}
