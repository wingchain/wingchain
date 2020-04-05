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

use std::path::PathBuf;

use parking_lot::RwLock;
use rocksdb::{
	BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, Options, ReadOptions, WriteBatch,
	WriteOptions, DB as RocksDB,
};
use smallvec::SmallVec;

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
	pub fn open(path: &PathBuf) -> errors::Result<DB> {
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
						let _ = db.create_cf(name, &gen_cf_opts(col_count, &block_opts))?;
					}
					Ok(db)
				}
				err => err,
			},
			ok => ok,
		}?;

		let db = Self {
			db: RwLock::new(rocksdb),
			db_opts,
			write_opts,
			read_opts,
			block_opts,
		};

		Ok(db)
	}

	pub fn get(&self, col: u32, key: &[u8]) -> errors::Result<Option<DBValue>> {
		let ref db = *self.db.read();
		let cf = Self::get_cf(&db, col);

		let result = db.get_cf_opt(cf, key, &self.read_opts)?;

		Ok(result)
	}

	pub fn write(&self, transaction: DBTransaction) -> errors::Result<()> {
		let ref db = *self.db.write();

		let ops = transaction.ops;
		let mut batch = WriteBatch::default();
		for op in ops {
			match op {
				DBOp::Insert { col, key, value } => {
					let cf = Self::get_cf(&db, col);
					batch.put_cf(cf, &key, &value)?;
				}
				DBOp::Delete { col, key } => {
					let cf = Self::get_cf(&db, col);
					batch.delete_cf(cf, &key)?;
				}
			};
		}
		db.write_opt(batch, &self.write_opts)?;
		Ok(())
	}

	pub fn transaction(&self) -> DBTransaction {
		DBTransaction::new()
	}

	fn get_cf(db: &RocksDB, col: u32) -> &ColumnFamily {
		let col_name = columns::COLUMN_NAMES[col as usize];
		let cf = db.cf_handle(col_name).expect("col name should exist");
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
}

pub mod columns {
	/// column names, which should be corresponding to the following const
	pub const COLUMN_NAMES: [&str; 7] = [
		"global",
		"block_hash",
		"header",
		"meta_state",
		"meta_txs",
		"state",
		"txs",
	];

	/// see global_key
	pub const GLOBAL: u32 = 0;

	/// block number to block hash
	pub const BLOCK_HASH: u32 = 1;

	/// block hash to header
	pub const HEADER: u32 = 2;

	/// meta state trie
	pub const META_STATE: u32 = 3;

	/// meta transactions
	pub const META_TXS: u32 = 4;

	/// state trie
	pub const STATE: u32 = 5;

	/// transactions
	pub const TXS: u32 = 6;
}

pub mod global_key {
	/// The best number of the chain
	pub const BEST_NUMBER: &[u8] = b"best_number";
	/// spec
	pub const SPEC: &[u8] = b"spec";
}

pub type DBKey = SmallVec<[u8; 32]>;
pub type DBValue = Vec<u8>;
