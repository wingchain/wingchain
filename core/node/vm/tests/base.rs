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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{DsaImpl, KeyPairImpl};
use crypto::hash::{Hash as HashT, HashImpl};
use node_vm::errors::{ContractError, VMResult};
use node_vm::{Mode, VMCallEnv, VMConfig, VMContext, VMContextEnv, VMContractEnv, VM};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Balance, DBKey, DBValue, Hash, PublicKey, SecretKey};

pub fn test_accounts() -> (
	(SecretKey, PublicKey, KeyPairImpl, Address),
	(SecretKey, PublicKey, KeyPairImpl, Address),
) {
	let address = Arc::new(AddressImpl::Blake2b160);
	let dsa = Arc::new(DsaImpl::Ed25519);
	utils_test::test_accounts(dsa, address)
}

pub fn vm_execute(
	code: &[u8],
	context: Rc<dyn VMContext>,
	mode: Mode,
	method: &str,
	input: &str,
) -> VMResult<String> {
	let hash = Arc::new(HashImpl::Blake2b256);
	let config = VMConfig::default();

	let vm = VM::new(config, context).unwrap();

	let code_hash = {
		let mut out = vec![0; hash.length().into()];
		hash.hash(&mut out, &code);
		Hash(out)
	};
	let method = method.as_bytes().to_vec();
	let input = input.as_bytes().to_vec();
	let result = vm.execute(mode, &code_hash, &code, method, input)?;
	let result: String = String::from_utf8(result).map_err(|_| ContractError::Deserialize)?;
	Ok(result)
}

const SEPARATOR: &[u8] = b"_";

pub struct TestStorage {
	payload: HashMap<DBKey, DBValue>,
}

impl TestStorage {
	pub fn new() -> Self {
		TestStorage {
			payload: HashMap::new(),
		}
	}

	#[allow(dead_code)]
	pub fn mint(&mut self, items: Vec<(Address, Balance)>) -> VMResult<()> {
		for (address, balance) in items {
			self.payload_storage_map_set(b"balance", b"balance", &address, &balance)?;
		}
		Ok(())
	}

	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
		Ok(self.payload.get(&DBKey::from_slice(key)).cloned())
	}

	fn payload_set(&mut self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		match value {
			Some(value) => {
				self.payload.insert(DBKey::from_slice(key), value);
			}
			None => {
				self.payload.remove(&DBKey::from_slice(key));
			}
		}
		Ok(())
	}

	fn payload_storage_map_get<K: Encode, V: Decode>(
		&self,
		module_name: &[u8],
		storage_name: &[u8],
		key: &K,
	) -> VMResult<Option<V>> {
		let key = codec::encode(key)?;
		let key = &[module_name, SEPARATOR, storage_name, SEPARATOR, &key].concat();
		let value = self.payload_get(key)?;
		let value = match value {
			Some(value) => {
				let value = codec::decode(&value[..])?;
				Ok(Some(value))
			}
			None => Ok(None),
		};

		value
	}
	fn payload_storage_map_set<K: Encode, V: Encode>(
		&mut self,
		module_name: &[u8],
		storage_name: &[u8],
		key: &K,
		value: &V,
	) -> VMResult<()> {
		let key = codec::encode(key)?;
		let key = &[module_name, SEPARATOR, storage_name, SEPARATOR, &key].concat();

		let value = codec::encode(value)?;
		self.payload_set(key, Some(value))?;
		Ok(())
	}
}

#[derive(Clone)]
pub struct TestVMContext {
	env: Rc<VMContextEnv>,
	call_env: Rc<VMCallEnv>,
	contract_env: Rc<VMContractEnv>,
	storage: Rc<RefCell<TestStorage>>,
	buffer: Rc<RefCell<HashMap<DBKey, Option<DBValue>>>>,
	events: Rc<RefCell<Vec<Vec<u8>>>>,
	hash: Arc<HashImpl>,
	address: Arc<AddressImpl>,
}

impl TestVMContext {
	pub fn new(
		sender_address: Address,
		pay_value: Balance,
		storage: Rc<RefCell<TestStorage>>,
	) -> Self {
		let hash = Arc::new(HashImpl::Blake2b256);
		let address = Arc::new(AddressImpl::Blake2b160);

		let tx_hash = {
			let mut out = vec![0u8; hash.length().into()];
			hash.hash(&mut out, &vec![1]);
			Hash(out)
		};
		let contract_address = {
			let mut out = vec![0u8; address.length().into()];
			address.address(&mut out, &vec![1]);
			Address(out)
		};
		let context = TestVMContext {
			env: Rc::new(VMContextEnv {
				number: 10,
				timestamp: 12345,
			}),
			call_env: Rc::new(VMCallEnv { tx_hash }),
			contract_env: Rc::new(VMContractEnv {
				contract_address,
				sender_address,
				pay_value,
			}),
			storage,
			buffer: Rc::new(RefCell::new(HashMap::new())),
			events: Rc::new(RefCell::new(Vec::new())),
			hash: hash.clone(),
			address: address.clone(),
		};

		context
	}
}

impl VMContext for TestVMContext {
	fn env(&self) -> Rc<VMContextEnv> {
		self.env.clone()
	}
	fn call_env(&self) -> Rc<VMCallEnv> {
		self.call_env.clone()
	}
	fn contract_env(&self) -> Rc<VMContractEnv> {
		self.contract_env.clone()
	}
	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
		if let Some(value) = self.buffer.borrow().get(&DBKey::from_slice(key)) {
			return Ok(value.clone());
		}
		self.storage.borrow().payload_get(key)
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		let mut buffer = self.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		let buffer = self.buffer.borrow_mut().drain().collect();
		Ok(buffer)
	}
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		let mut storage = self.storage.borrow_mut();

		for (k, v) in items {
			storage.payload_set(k.as_slice(), v)?;
		}
		Ok(())
	}
	fn emit_event(&self, event: Vec<u8>) -> VMResult<()> {
		self.events.borrow_mut().push(event);
		Ok(())
	}
	fn drain_events(&self) -> VMResult<Vec<Vec<u8>>> {
		let events = self.events.borrow_mut().drain(..).collect();
		Ok(events)
	}
	fn hash(&self, data: &[u8]) -> VMResult<Hash> {
		let mut out = vec![0u8; self.hash.length().into()];
		self.hash.hash(&mut out, data);
		Ok(Hash(out))
	}
	fn address(&self, data: &[u8]) -> VMResult<Address> {
		let mut out = vec![0u8; self.address.length().into()];
		self.address.address(&mut out, data);
		Ok(Address(out))
	}
	fn validate_address(&self, address: &Address) -> VMResult<()> {
		let address_len: usize = self.address.length().into();
		if address.0.len() != address_len {
			return Err(ContractError::InvalidAddress.into());
		}
		Ok(())
	}
	fn balance_get(&self, address: &Address) -> VMResult<Balance> {
		let balance: Option<Balance> = self
			.storage
			.borrow()
			.payload_storage_map_get(b"balance", b"balance", address)?;
		let balance = balance.unwrap_or(0);
		Ok(balance)
	}
	fn balance_transfer(
		&self,
		sender: &Address,
		recipient: &Address,
		value: Balance,
	) -> VMResult<()> {
		let mut sender_balance = self.balance_get(sender)?;
		let mut recipient_balance = self.balance_get(recipient)?;

		if sender_balance < value {
			return Err(ContractError::Transfer.into());
		}

		sender_balance -= value;
		recipient_balance += value;

		self.storage.borrow_mut().payload_storage_map_set(
			b"balance",
			b"balance",
			sender,
			&sender_balance,
		)?;
		self.storage.borrow_mut().payload_storage_map_set(
			b"balance",
			b"balance",
			recipient,
			&recipient_balance,
		)?;
		Ok(())
	}
}
