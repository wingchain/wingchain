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

/// Executor primitives for modules
use std::marker::PhantomData;
use std::sync::Arc;

use codec::{Decode, Encode};
use primitives::{codec, Address, BlockNumber, Call, DBKey, DBValue, Event, Hash};

pub use crate::errors::{ModuleError, ModuleResult, OpaqueModuleResult};

pub mod errors;

/// Separator to build kv db key
pub const SEPARATOR: &[u8] = b"_";

pub trait Module {
	type C: Context;
	type U: Util;
	/// Specify whether the module is meta module
	const META_MODULE: bool = false;

	/// Specify the kv db key prefix
	const STORAGE_KEY: &'static [u8];

	fn new(context: Self::C, util: Self::U) -> Self;

	/// check if the call is a write call, a transaction should be built by a write call
	fn is_write_call(call: &Call) -> Option<bool>;

	/// validate the call
	fn validate_call(&self, sender: Option<&Address>, call: &Call) -> ModuleResult<()>;

	/// execute the call
	fn execute_call(&self, sender: Option<&Address>, call: &Call) -> OpaqueModuleResult;
}

/// Env variables for a block
#[derive(Clone)]
pub struct ContextEnv {
	pub number: BlockNumber,
	pub timestamp: u64,
}

/// Env variables for a call
#[derive(Clone)]
pub struct CallEnv {
	pub tx_hash: Option<Hash>,
}

impl Default for CallEnv {
	fn default() -> Self {
		CallEnv { tx_hash: None }
	}
}

pub trait Context: Clone {
	/// Env for a context
	fn env(&self) -> Arc<ContextEnv>;
	/// Env for a call
	fn call_env(&self) -> Arc<CallEnv>;
	/// get meta state
	fn meta_get(&self, key: &[u8]) -> ModuleResult<Option<DBValue>>;
	/// set meta state (only save into tx buffer)
	fn meta_set(&self, key: &[u8], value: Option<DBValue>) -> ModuleResult<()>;
	/// drain meta tx buffer
	fn meta_drain_tx_buffer(&self) -> ModuleResult<Vec<(DBKey, Option<DBValue>)>>;
	/// apply meter
	fn meta_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> ModuleResult<()>;
	/// get payload state
	fn payload_get(&self, key: &[u8]) -> ModuleResult<Option<DBValue>>;
	/// set payload state (only save into tx buffer)
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> ModuleResult<()>;
	/// drain payload tx buffer
	fn payload_drain_tx_buffer(&self) -> ModuleResult<Vec<(DBKey, Option<DBValue>)>>;
	/// apply payload
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> ModuleResult<()>;
	/// emit an event
	fn emit_event(&self, event: Event) -> ModuleResult<()>;
	/// drain tx events
	fn drain_tx_events(&self) -> ModuleResult<Vec<Event>>;
	/// apply events
	fn apply_events(&self, items: Vec<Event>) -> ModuleResult<()>;
}

pub trait Util: Clone {
	/// compute hash
	fn hash(&self, data: &[u8]) -> ModuleResult<Hash>;
	/// compute address
	fn address(&self, data: &[u8]) -> ModuleResult<Address>;
	/// validate address
	fn validate_address(&self, address: &Address) -> ModuleResult<()>;
}

/// Storage type for module
/// module_storage_key + separator + storage_key => value
pub struct StorageValue<T, M>
where
	T: Encode + Decode,
	M: Module,
{
	context: M::C,
	meta_module: bool,
	key: Vec<u8>,
	phantom: PhantomData<T>,
}

impl<T, M> StorageValue<T, M>
where
	T: Encode + Decode,
	M: Module,
{
	pub fn new(context: M::C, storage_key: &'static [u8]) -> Self {
		let key = [M::STORAGE_KEY, SEPARATOR, storage_key].concat();
		let meta_module = M::META_MODULE;
		Self {
			context,
			meta_module,
			key,
			phantom: Default::default(),
		}
	}

	pub fn get(&self) -> ModuleResult<Option<T>> {
		context_get(&self.context, self.meta_module, &self.key)
	}

	pub fn set(&self, value: &T) -> ModuleResult<()> {
		context_set(&self.context, self.meta_module, &self.key, value)
	}

	pub fn delete(&self) -> ModuleResult<()> {
		context_delete(&self.context, self.meta_module, &self.key)
	}
}

/// Storage type for module
/// module_storage_key + separator + storage_key + separator + key => value
pub struct StorageMap<K, V, M>
where
	K: Encode + Decode,
	V: Encode + Decode,
	M: Module,
{
	context: M::C,
	meta_module: bool,
	key: Vec<u8>,
	phantom: PhantomData<(K, V)>,
}

impl<K, V, M> StorageMap<K, V, M>
where
	K: Encode + Decode,
	V: Encode + Decode,
	M: Module,
{
	pub fn new(context: M::C, storage_key: &'static [u8]) -> Self {
		let key = [M::STORAGE_KEY, SEPARATOR, storage_key].concat();
		let meta_module = M::META_MODULE;
		Self {
			context,
			meta_module,
			key,
			phantom: Default::default(),
		}
	}

	pub fn get(&self, key: &K) -> ModuleResult<Option<V>> {
		let key = codec::encode(&key)?;
		self.raw_get(&key)
	}

	pub fn raw_get(&self, key: &[u8]) -> ModuleResult<Option<V>> {
		let key = &[&self.key, SEPARATOR, &key].concat();
		context_get(&self.context, self.meta_module, key)
	}

	pub fn set(&self, key: &K, value: &V) -> ModuleResult<()> {
		let key = codec::encode(&key)?;
		self.raw_set(&key, value)
	}

	pub fn raw_set(&self, key: &[u8], value: &V) -> ModuleResult<()> {
		let key = &[&self.key, SEPARATOR, &key].concat();
		context_set(&self.context, self.meta_module, key, value)
	}

	pub fn delete(&self, key: &K) -> ModuleResult<()> {
		let key = codec::encode(&key)?;
		self.raw_delete(&key)
	}

	pub fn raw_delete(&self, key: &[u8]) -> ModuleResult<()> {
		let key = &[&self.key, SEPARATOR, &key].concat();
		context_delete(&self.context, self.meta_module, key)
	}
}

fn context_get<C: Context, V: Decode>(
	context: &C,
	meta_module: bool,
	key: &[u8],
) -> ModuleResult<Option<V>> {
	let value = match meta_module {
		true => context.meta_get(key),
		false => context.payload_get(key),
	};
	match value {
		Ok(value) => match value {
			Some(value) => {
				let value = codec::decode(&value[..])?;
				Ok(Some(value))
			}
			None => Ok(None),
		},
		Err(err) => Err(err),
	}
}

fn context_set<C: Context, V: Encode>(
	context: &C,
	meta_module: bool,
	key: &[u8],
	value: &V,
) -> ModuleResult<()> {
	let value = codec::encode(value)?;
	let value = Some(value);
	match meta_module {
		true => context.meta_set(&key, value),
		false => context.payload_set(&key, value),
	}
}

fn context_delete<C: Context>(context: &C, meta_module: bool, key: &[u8]) -> ModuleResult<()> {
	match meta_module {
		true => context.meta_set(key, None),
		false => context.payload_set(key, None),
	}
}

/// Used by call which need not params
#[derive(Encode, Decode)]
pub struct EmptyParams;
