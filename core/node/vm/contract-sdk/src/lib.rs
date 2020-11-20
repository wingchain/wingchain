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

//! Contract sdk

#[allow(unused_imports)]
#[macro_use]
extern crate contract_sdk_macro;

use std::rc::Rc;

use byteorder::ByteOrder;
use serde::de::DeserializeOwned;
use serde::export::PhantomData;
use serde::Serialize;

#[doc(hidden)]
pub use contract_sdk_macro::*;

#[allow(unused_imports)]
pub use contract_sdk_primitives::*;

pub mod import;

pub struct Context {
	env: Rc<ContextEnv>,
	call_env: Rc<CallEnv>,
	contract_env: Rc<ContractEnv>,
}

pub struct Util;

impl Context {
	pub fn new() -> ContractResult<Self> {
		let number = import::env_block_number();
		let timestamp = import::env_block_timestamp();

		let tx_hash_len = import::env_tx_hash_len();
		let tx_hash = vec![0u8; tx_hash_len as usize];
		import::env_tx_hash_read(tx_hash.as_ptr() as _);
		let tx_hash = Hash(tx_hash);

		let contract_address_len = import::env_contract_address_len();
		let contract_address = vec![0u8; contract_address_len as usize];
		import::env_contract_address_read(contract_address.as_ptr() as _);
		let contract_address = Address(contract_address);

		let sender_address_len = import::env_sender_address_len();
		let sender_address = vec![0u8; sender_address_len as usize];
		import::env_sender_address_read(sender_address.as_ptr() as _);
		let sender_address = Address(sender_address);

		let pay_value = import::env_pay_value();

		let context = Self {
			env: Rc::new(ContextEnv { number, timestamp }),
			call_env: Rc::new(CallEnv { tx_hash }),
			contract_env: Rc::new(ContractEnv {
				contract_address,
				sender_address,
				pay_value,
			}),
		};
		Ok(context)
	}
	pub fn env(&self) -> ContractResult<Rc<ContextEnv>> {
		Ok(self.env.clone())
	}
	pub fn call_env(&self) -> ContractResult<Rc<CallEnv>> {
		Ok(self.call_env.clone())
	}
	pub fn contract_env(&self) -> ContractResult<Rc<ContractEnv>> {
		Ok(self.contract_env.clone())
	}
	pub fn emit_event<E: Serialize>(&self, event: E) -> ContractResult<()> {
		let event = serde_json::to_vec(&event).map_err(|_| ContractError::Serialize)?;
		import::event_write(event.len() as _, event.as_ptr() as _);
		Ok(())
	}
	pub fn balance_get(&self, address: &Address) -> ContractResult<Balance> {
		let address = &address.0;
		let balance = import::balance_read(address.len() as _, address.as_ptr() as _);
		Ok(balance)
	}
	pub fn balance_transfer(
		&self,
		recipient_address: &Address,
		value: Balance,
	) -> ContractResult<()> {
		let recipient_address = &recipient_address.0;
		import::balance_transfer(
			recipient_address.len() as _,
			recipient_address.as_ptr() as _,
			value,
		);
		Ok(())
	}
}

impl Util {
	pub fn new() -> ContractResult<Self> {
		Ok(Util)
	}
	pub fn hash(&self, data: &[u8]) -> ContractResult<Hash> {
		let hash_len = import::compute_hash_len();
		let result = vec![0u8; hash_len as usize];
		import::compute_hash(data.len() as _, data.as_ptr() as _, result.as_ptr() as _);
		Ok(Hash(result))
	}
	pub fn address(&self, data: &[u8]) -> ContractResult<Address> {
		let address_len = import::compute_address_len();
		let result = vec![0u8; address_len as usize];
		import::compute_address(data.len() as _, data.as_ptr() as _, result.as_ptr() as _);
		Ok(Address(result))
	}
}

pub struct StorageValue<T>
where
	T: Serialize + DeserializeOwned,
{
	key: Vec<u8>,
	phantom: PhantomData<T>,
}

impl<T> StorageValue<T>
where
	T: Serialize + DeserializeOwned,
{
	pub fn new(storage_key: &'static [u8]) -> Self {
		StorageValue {
			key: storage_key.to_vec(),
			phantom: Default::default(),
		}
	}

	pub fn get(&self) -> ContractResult<Option<T>> {
		storage_get(&self.key)
	}

	pub fn set(&self, value: &T) -> ContractResult<()> {
		storage_set(&self.key, value)
	}

	pub fn delete(&self) -> ContractResult<()> {
		storage_delete(&self.key)
	}
}

const SEPARATOR: &[u8] = b"_";

pub struct StorageMap<T>
where
	T: Serialize + DeserializeOwned,
{
	key: Vec<u8>,
	phantom: PhantomData<T>,
}

impl<T> StorageMap<T>
where
	T: Serialize + DeserializeOwned,
{
	pub fn new(storage_key: &'static [u8]) -> Self {
		StorageMap {
			key: storage_key.to_vec(),
			phantom: Default::default(),
		}
	}

	pub fn get(&self, key: &[u8]) -> ContractResult<Option<T>> {
		let key = &[&self.key, SEPARATOR, &key].concat();
		storage_get(&key)
	}

	pub fn set(&self, key: &[u8], value: &T) -> ContractResult<()> {
		let key = &[&self.key, SEPARATOR, &key].concat();
		storage_set(&key, value)
	}

	pub fn delete(&self, key: &[u8]) -> ContractResult<()> {
		let key = &[&self.key, SEPARATOR, &key].concat();
		storage_delete(&key)
	}
}

pub struct ContextEnv {
	pub number: BlockNumber,
	pub timestamp: u64,
}

pub struct CallEnv {
	pub tx_hash: Hash,
}

pub struct ContractEnv {
	pub contract_address: Address,
	pub sender_address: Address,
	pub pay_value: Balance,
}

fn storage_get<V: DeserializeOwned>(key: &[u8]) -> ContractResult<Option<V>> {
	let exist_len = import::storage_exist_len(key.len() as _, key.as_ptr() as _);

	let mut buffer = vec![0u8; 8];
	byteorder::LittleEndian::write_u64(&mut buffer, exist_len);
	let mut exist_len = [0; 2];
	byteorder::LittleEndian::read_u32_into(&buffer, &mut exist_len);
	let (exist, len) = (exist_len[0], exist_len[1]);

	let value = match exist {
		1 => {
			let value = vec![0u8; len as usize];
			import::storage_read(key.len() as _, key.as_ptr() as _, value.as_ptr() as _);
			Some(value)
		}
		_ => None,
	};

	let value = match value {
		Some(value) => serde_json::from_slice(&value).map_err(|_| ContractError::Deserialize)?,
		None => None,
	};
	Ok(value)
}

fn storage_set<V: Serialize>(key: &[u8], value: &V) -> ContractResult<()> {
	let value = serde_json::to_vec(value).map_err(|_| ContractError::Serialize)?;
	import::storage_write(
		key.len() as _,
		key.as_ptr() as _,
		1,
		value.len() as _,
		value.as_ptr() as _,
	);
	Ok(())
}

fn storage_delete(key: &[u8]) -> ContractResult<()> {
	import::storage_write(key.len() as _, key.as_ptr() as _, 0, 0, 0);
	Ok(())
}
