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

//! State db maintaining all the historical trie

use std::iter::IntoIterator;
use std::sync::Arc;

use hash_db::{AsHashDB, HashDB, Hasher, Prefix};
use log::warn;
use memory_db::{KeyFunction, PrefixedKey};
use reference_trie::ReferenceTrieStream;
use trie_db::{Trie, TrieMut};

use crypto::hash::{Hash as HashT, HashImpl};
use crypto::HashLength;
use node_db::{DBTransaction, DB};
use primitives::codec;
use primitives::errors::CommonResult;
use primitives::{DBKey, DBValue};
pub use trie::{
	DefaultMemoryDB, DefaultTrieDB, DefaultTrieDBMut, TrieHasher20, TrieHasher32, TrieHasher64,
	H512,
};

use crate::errors::parse_trie_error;
use crate::trie::load_hasher;

pub mod errors;
mod trie;

pub struct StateDB {
	db: Arc<DB>,
	pub db_column: u32,
	#[allow(dead_code)]
	hasher: Arc<HashImpl>,
	hash_length: HashLength,
}

impl StateDB {
	/// Create a new statedb
	pub fn new(db: Arc<DB>, db_column: u32, hasher: Arc<HashImpl>) -> CommonResult<Self> {
		load_hasher(hasher.clone())?;

		let hash_length = hasher.length();
		Ok(Self {
			db,
			db_column,
			hasher,
			hash_length,
		})
	}

	/// Get the default trie root
	pub fn default_root(&self) -> Vec<u8> {
		match self.hash_length {
			HashLength::HashLength20 => self.default_root_for_hasher::<TrieHasher20>().to_vec(),
			HashLength::HashLength32 => self.default_root_for_hasher::<TrieHasher32>().to_vec(),
			HashLength::HashLength64 => self
				.default_root_for_hasher::<TrieHasher64>()
				.as_ref()
				.to_vec(),
		}
	}

	/// Get the value from the trie
	/// by trie root and key
	pub fn get(&self, root: &[u8], key: &[u8]) -> CommonResult<Option<DBValue>> {
		let result = match self.hash_length {
			HashLength::HashLength20 => {
				let mut typed_root = [0u8; 20];
				typed_root.copy_from_slice(&root);
				self.get_for_hasher::<TrieHasher20>(typed_root, key)
			}
			HashLength::HashLength32 => {
				let mut typed_root = [0u8; 32];
				typed_root.copy_from_slice(&root);
				self.get_for_hasher::<TrieHasher32>(typed_root, key)
			}
			HashLength::HashLength64 => {
				let mut typed_root = [0u8; 64];
				typed_root.copy_from_slice(&root);
				let typed_root = H512::from(typed_root);
				self.get_for_hasher::<TrieHasher64>(typed_root, key)
			}
		};
		result
	}

	/// Get the statedb statement by trie root
	/// by which one can get a statedb getter
	pub fn prepare_stmt(&self, root: &[u8]) -> CommonResult<StateDBStmt> {
		let new = self.default_root() == root;
		Ok(StateDBStmt::new(
			self.db.clone(),
			self.db_column,
			root,
			new,
			&self.hash_length,
		))
	}

	/// Get a statedb getter by statedb statement
	pub fn prepare_get(stmt: &StateDBStmt) -> CommonResult<StateDBGetter> {
		let result = match stmt {
			StateDBStmt::Hasher20(stmt) => {
				StateDBGetter::Hasher20(Self::prepare_get_for_hasher(&stmt)?)
			}
			StateDBStmt::Hasher32(stmt) => {
				StateDBGetter::Hasher32(Self::prepare_get_for_hasher(&stmt)?)
			}
			StateDBStmt::Hasher64(stmt) => {
				StateDBGetter::Hasher64(Self::prepare_get_for_hasher(&stmt)?)
			}
		};
		Ok(result)
	}

	/// Try update with batch key-values,
	/// return new trie root and db transaction to update the state column of DB
	pub fn prepare_update<'a, I>(
		&self,
		root: &[u8],
		data: I,
	) -> CommonResult<(Vec<u8>, DBTransaction)>
	where
		I: Iterator<Item = (&'a DBKey, &'a Option<DBValue>)>,
	{
		let result = match self.hash_length {
			HashLength::HashLength20 => {
				let mut typed_root = [0u8; 20];
				typed_root.copy_from_slice(&root);

				self.prepare_update_for_hasher::<_, TrieHasher20>(typed_root, data)
					.map(|(root, transaction)| (root.to_vec(), transaction))?
			}
			HashLength::HashLength32 => {
				let mut typed_root = [0u8; 32];
				typed_root.copy_from_slice(&root);

				self.prepare_update_for_hasher::<_, TrieHasher32>(typed_root, data)
					.map(|(root, transaction)| (root.to_vec(), transaction))?
			}
			HashLength::HashLength64 => {
				let mut typed_root = [0u8; 64];
				typed_root.copy_from_slice(&root);
				let typed_root = H512::from(typed_root);

				self.prepare_update_for_hasher::<_, TrieHasher64>(typed_root, data)
					.map(|(root, transaction)| (root.as_bytes().to_vec(), transaction))?
			}
		};
		Ok(result)
	}
}

/// private impl
impl StateDB {
	fn default_root_for_hasher<H>(&self) -> H::Out
	where
		H: Hasher,
	{
		H::Out::default()
	}

	fn get_for_hasher<H>(&self, root: H::Out, key: &[u8]) -> CommonResult<Option<DBValue>>
	where
		H: Hasher,
	{
		let buffer = DefaultMemoryDB::<H>::default();
		let proxy = ProxyHashDB {
			db: self.db.clone(),
			db_column: self.db_column,
			buffer,
		};

		let triedb = DefaultTrieDB::<H>::new(&proxy, &root).map_err(parse_trie_error)?;

		let result = triedb.get(&key).map_err(parse_trie_error)?;

		Ok(result)
	}

	fn prepare_get_for_hasher<H>(
		stmt: &StateDBStmtForHasher<H>,
	) -> CommonResult<StateDBGetterForHasher<H>>
	where
		H: Hasher,
	{
		let triedb = match stmt.new {
			false => {
				Some(DefaultTrieDB::<H>::new(&stmt.proxy, &stmt.root).map_err(parse_trie_error)?)
			}
			true => None,
		};
		let statedb_getter = StateDBGetterForHasher { triedb };
		Ok(statedb_getter)
	}

	fn prepare_update_for_hasher<'a, I, H>(
		&self,
		mut root: H::Out,
		data: I,
	) -> CommonResult<(H::Out, DBTransaction)>
	where
		I: Iterator<Item = (&'a DBKey, &'a Option<DBValue>)>,
		H: Hasher,
	{
		let new = self.default_root_for_hasher::<H>() == root;

		let buffer = DefaultMemoryDB::<H>::default();
		let mut proxy = ProxyHashDB {
			db: self.db.clone(),
			db_column: self.db_column,
			buffer: buffer,
		};

		{
			let mut triedb = match new {
				true => DefaultTrieDBMut::<H>::new(&mut proxy, &mut root),
				false => DefaultTrieDBMut::<H>::from_existing(&mut proxy, &mut root)
					.map_err(parse_trie_error)?,
			};

			// apply data to trie
			for (k, v) in data {
				match v {
					Some(v) => {
						triedb.insert(&k, &v).map_err(parse_trie_error)?;
					}
					None => {
						triedb.remove(&k).map_err(parse_trie_error)?;
					}
				}
			}
		}

		// extract buffer to transaction
		let mut transaction = self.db.transaction();
		for (k, (v, rc)) in proxy.buffer.drain() {
			// only apply insert
			if rc > 0 {
				transaction.put_owned(self.db_column, DBKey::from_slice(&k), v);
			}
		}

		Ok((root, transaction))
	}
}

pub enum StateDBStmt {
	Hasher20(StateDBStmtForHasher<TrieHasher20>),
	Hasher32(StateDBStmtForHasher<TrieHasher32>),
	Hasher64(StateDBStmtForHasher<TrieHasher64>),
}

impl StateDBStmt {
	pub fn new(
		db: Arc<DB>,
		db_column: u32,
		root: &[u8],
		new: bool,
		hash_length: &HashLength,
	) -> Self {
		match hash_length {
			HashLength::HashLength20 => {
				let mut typed_root = [0u8; 20];
				typed_root.copy_from_slice(&root);
				Self::Hasher20(StateDBStmtForHasher::<TrieHasher20>::new(
					db, db_column, typed_root, new,
				))
			}
			HashLength::HashLength32 => {
				let mut typed_root = [0u8; 32];
				typed_root.copy_from_slice(&root);
				Self::Hasher32(StateDBStmtForHasher::<TrieHasher32>::new(
					db, db_column, typed_root, new,
				))
			}
			HashLength::HashLength64 => {
				let mut typed_root = [0u8; 64];
				typed_root.copy_from_slice(&root);
				let typed_root = H512::from(typed_root);
				Self::Hasher64(StateDBStmtForHasher::<TrieHasher64>::new(
					db, db_column, typed_root, new,
				))
			}
		}
	}
}

pub struct StateDBStmtForHasher<H>
where
	H: Hasher,
{
	proxy: ProxyHashDB<H>,
	root: H::Out,
	new: bool,
}

impl<H> StateDBStmtForHasher<H>
where
	H: Hasher,
{
	pub fn new(db: Arc<DB>, db_column: u32, root: H::Out, new: bool) -> Self {
		let buffer = DefaultMemoryDB::<H>::default();
		let proxy = ProxyHashDB {
			db,
			db_column,
			buffer,
		};
		Self { proxy, root, new }
	}
}

pub enum StateDBGetter<'a> {
	Hasher20(StateDBGetterForHasher<'a, TrieHasher20>),
	Hasher32(StateDBGetterForHasher<'a, TrieHasher32>),
	Hasher64(StateDBGetterForHasher<'a, TrieHasher64>),
}

pub struct StateDBGetterForHasher<'a, H>
where
	H: Hasher,
{
	triedb: Option<DefaultTrieDB<'a, H>>,
}

impl<'a> StateDBGetter<'a> {
	pub fn get(&self, key: &[u8]) -> CommonResult<Option<DBValue>> {
		let result = match self {
			Self::Hasher20(g) => match &g.triedb {
				Some(triedb) => triedb.get(key).map_err(parse_trie_error)?,
				None => None,
			},
			Self::Hasher32(g) => match &g.triedb {
				Some(triedb) => triedb.get(key).map_err(parse_trie_error)?,
				None => None,
			},
			Self::Hasher64(g) => match &g.triedb {
				Some(triedb) => triedb.get(key).map_err(parse_trie_error)?,
				None => None,
			},
		};
		Ok(result)
	}
}

struct ProxyHashDB<H: Hasher> {
	db: Arc<DB>,
	db_column: u32,
	buffer: DefaultMemoryDB<H>,
}

impl<H: Hasher> HashDB<H, DBValue> for ProxyHashDB<H> {
	fn get(&self, key: &H::Out, prefix: Prefix) -> Option<DBValue> {
		if let Some(val) = hash_db::HashDB::get(&self.buffer, key, prefix) {
			Some(val)
		} else {
			let key = PrefixedKey::<H>::key(key, prefix);
			match self.db.get(self.db_column, &key) {
				Ok(x) => x,
				Err(e) => {
					warn!("Failed to read from DB: {}", e);
					None
				}
			}
		}
	}

	fn contains(&self, key: &H::Out, prefix: Prefix) -> bool {
		HashDB::get(self, key, prefix).is_some()
	}

	fn insert(&mut self, prefix: Prefix, value: &[u8]) -> H::Out {
		hash_db::HashDB::insert(&mut self.buffer, prefix, value)
	}

	fn emplace(&mut self, key: H::Out, prefix: Prefix, value: DBValue) {
		hash_db::HashDB::emplace(&mut self.buffer, key, prefix, value)
	}

	fn remove(&mut self, key: &H::Out, prefix: Prefix) {
		hash_db::HashDB::remove(&mut self.buffer, key, prefix)
	}
}

impl<'a, H: Hasher> hash_db::HashDBRef<H, DBValue> for ProxyHashDB<H> {
	fn get(&self, key: &H::Out, prefix: Prefix) -> Option<DBValue> {
		hash_db::HashDB::get(self, key, prefix)
	}

	fn contains(&self, key: &H::Out, prefix: Prefix) -> bool {
		hash_db::HashDB::contains(self, key, prefix)
	}
}

impl<H: Hasher> AsHashDB<H, DBValue> for ProxyHashDB<H> {
	fn as_hash_db<'b>(&'b self) -> &'b (dyn hash_db::HashDB<H, DBValue> + 'b) {
		self
	}
	fn as_hash_db_mut<'b>(&'b mut self) -> &'b mut (dyn hash_db::HashDB<H, DBValue> + 'b) {
		self
	}
}

pub struct TrieRoot {
	hash_length: HashLength,
}

impl TrieRoot {
	pub fn new(hasher: Arc<HashImpl>) -> CommonResult<Self> {
		let hash_length = hasher.length();
		load_hasher(hasher)?;

		Ok(Self { hash_length })
	}

	pub fn calc_trie_root<I, A, B>(&self, input: I) -> Vec<u8>
	where
		I: IntoIterator<Item = (A, B)>,
		A: AsRef<[u8]> + Ord,
		B: AsRef<[u8]>,
	{
		match self.hash_length {
			HashLength::HashLength20 => {
				let root =
					trie_root::trie_root::<TrieHasher20, ReferenceTrieStream, I, A, B>(input);
				root.to_vec()
			}
			HashLength::HashLength32 => {
				let root =
					trie_root::trie_root::<TrieHasher32, ReferenceTrieStream, I, A, B>(input);
				root.to_vec()
			}
			HashLength::HashLength64 => {
				let root =
					trie_root::trie_root::<TrieHasher64, ReferenceTrieStream, I, A, B>(input);
				root.as_bytes().to_vec()
			}
		}
	}

	pub fn calc_ordered_trie_root<I, A>(&self, input: I) -> Vec<u8>
	where
		I: IntoIterator<Item = A>,
		A: AsRef<[u8]>,
	{
		let input = input
			.into_iter()
			.enumerate()
			.map(|(k, v)| {
				(
					codec::encode(&(k as u32)).expect("u32 can be serialized"),
					v,
				)
			})
			.collect::<Vec<_>>();

		self.calc_trie_root(input)
	}
}
