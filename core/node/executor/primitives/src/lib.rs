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

use std::marker::PhantomData;

use parity_codec::{Decode, Encode};

use node_db::DBValue;
use primitives::Call;

pub mod errors;

const SEPARATOR: &[u8] = b"_";

pub trait Module<C>
where
	C: Context,
{
	const META_MODULE: bool = false;
	const STORAGE_KEY: &'static [u8];

	fn new(context: C) -> Self;

	fn validate_call(call: &Call) -> bool;

	fn execute_call(&self, call: &Call) -> Result<(), errors::Error>;
}

pub trait Context: Clone {
	fn meta_get(&self, key: &[u8]) -> errors::Result<Option<DBValue>>;
	fn meta_set(&self, key: &[u8], value: Option<DBValue>) -> errors::Result<()>;
	fn payload_get(&self, key: &[u8]) -> errors::Result<Option<DBValue>>;
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> errors::Result<()>;
}

pub struct StorageValue<T, C>
where
	T: Encode + Decode,
	C: Context,
{
	context: C,
	meta_module: bool,
	key: Vec<u8>,
	phantom: PhantomData<T>,
}

impl<T, C> StorageValue<T, C>
where
	T: Encode + Decode,
	C: Context,
{
	pub fn new<M: Module<C>>(context: C, storage_key: &'static [u8]) -> Self {
		let key = [M::STORAGE_KEY, SEPARATOR, storage_key].concat();
		let meta_module = M::META_MODULE;
		Self {
			context,
			meta_module,
			key,
			phantom: Default::default(),
		}
	}

	pub fn get(&self) -> errors::Result<Option<T>> {
		context_get(&self.context, self.meta_module, &self.key)
	}

	pub fn set(&self, value: &T) -> errors::Result<()> {
		context_set(&self.context, self.meta_module, &self.key, value)
	}

	pub fn delete(&self) -> errors::Result<()> {
		context_delete(&self.context, self.meta_module, &self.key)
	}
}

pub struct StorageMap<K, V, C>
where
	K: Encode + Decode,
	V: Encode + Decode,
	C: Context,
{
	context: C,
	meta_module: bool,
	key: Vec<u8>,
	phantom: PhantomData<(K, V)>,
}

impl<K, V, C> StorageMap<K, V, C>
where
	K: Encode + Decode,
	V: Encode + Decode,
	C: Context,
{
	pub fn new(
		context: C,
		meta_module: bool,
		module_key: &'static [u8],
		storage_key: &'static [u8],
	) -> Self {
		let key = [module_key, storage_key].concat();
		Self {
			context,
			meta_module,
			key,
			phantom: Default::default(),
		}
	}

	pub fn get(&self, key: K) -> errors::Result<Option<V>> {
		let key = &[&self.key, SEPARATOR, &key.encode()].concat();
		context_get(&self.context, self.meta_module, key)
	}

	pub fn set(&self, key: K, value: &V) -> errors::Result<()> {
		let key = &[&self.key, SEPARATOR, &key.encode()].concat();
		context_set(&self.context, self.meta_module, key, value)
	}

	pub fn delete(&self, key: K) -> errors::Result<()> {
		let key = &[&self.key, SEPARATOR, &key.encode()].concat();
		context_delete(&self.context, self.meta_module, key)
	}
}

fn context_get<C: Context, V: Decode>(
	context: &C,
	meta_module: bool,
	key: &[u8],
) -> errors::Result<Option<V>> {
	let value = match meta_module {
		true => context.meta_get(key),
		false => context.payload_get(key),
	};
	match value {
		Ok(value) => match value {
			Some(value) => {
				let value =
					Decode::decode(&mut &value[..]).map_err(|_| errors::ErrorKind::CodecError)?;
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
) -> errors::Result<()> {
	let value = Some(value.encode());
	match meta_module {
		true => context.meta_set(&key, value),
		false => context.payload_set(&key, value),
	}
}

fn context_delete<C: Context>(context: &C, meta_module: bool, key: &[u8]) -> errors::Result<()> {
	match meta_module {
		true => context.meta_set(key, None),
		false => context.payload_set(key, None),
	}
}
