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
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{DsaImpl, KeyPairImpl};
use crypto::hash::{Hash as HashT, HashImpl};
use node_vm::errors::{ContractError, VMResult};
use node_vm::{Mode, VMCallEnv, VMConfig, VMContext, VMContextEnv, VMContractEnv, VM};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Balance, DBKey, DBValue, Event, Hash, PublicKey, SecretKey};

pub fn test_accounts() -> (
	(SecretKey, PublicKey, KeyPairImpl, Address),
	(SecretKey, PublicKey, KeyPairImpl, Address),
) {
	let address = Arc::new(AddressImpl::Blake2b160);
	let dsa = Arc::new(DsaImpl::Ed25519);
	utils_test::test_accounts(dsa, address)
}

#[allow(dead_code)]
pub fn vm_validate(
	code: &[u8],
	mode: Mode,
	method: &str,
	input: &str,
	pay_value: Balance,
) -> VMResult<()> {
	let hash = Arc::new(HashImpl::Blake2b256);
	let config = VMConfig::default();

	let vm = VM::new(config);

	let code_hash = {
		let mut out = vec![0; hash.length().into()];
		hash.hash(&mut out, &code);
		Hash(out)
	};
	let method = method.as_bytes().to_vec();
	let input = input.as_bytes().to_vec();
	let result = vm.validate(&code_hash, &code, mode, method, input, pay_value)?;
	Ok(result)
}

pub fn vm_execute(
	code: &[u8],
	context: &dyn VMContext,
	mode: Mode,
	method: &str,
	input: &str,
	pay_value: Balance,
) -> VMResult<String> {
	let hash = Arc::new(HashImpl::Blake2b256);
	let config = VMConfig::default();

	let vm = VM::new(config);

	let code_hash = {
		let mut out = vec![0; hash.length().into()];
		hash.hash(&mut out, &code);
		Hash(out)
	};
	let method = method.as_bytes().to_vec();
	let input = input.as_bytes().to_vec();
	let result = vm.execute(&code_hash, &code, context, mode, method, input, pay_value)?;
	let result: String = String::from_utf8(result).map_err(|_| ContractError::Deserialize)?;
	Ok(result)
}

const SEPARATOR: &[u8] = b"_";

pub trait ExecutorContext: Clone {
	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>>;
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()>;
	fn payload_drain_tx_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>>;
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()>;
	fn emit_event(&self, event: Event) -> VMResult<()>;
	fn drain_tx_events(&self) -> VMResult<Vec<Event>>;
	fn apply_events(&self, items: Vec<Event>) -> VMResult<()>;
}

#[derive(Clone)]
pub struct TestExecutorContext {
	payload: Rc<RefCell<HashMap<DBKey, DBValue>>>,
	events: Rc<RefCell<Vec<Event>>>,
}

impl TestExecutorContext {
	pub fn new() -> Self {
		TestExecutorContext {
			payload: Rc::new(RefCell::new(HashMap::new())),
			events: Rc::new(RefCell::new(Vec::new())),
		}
	}
}

impl ExecutorContext for TestExecutorContext {
	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
		Ok(self.payload.borrow().get(&DBKey::from_slice(key)).cloned())
	}

	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		match value {
			Some(value) => {
				self.payload
					.borrow_mut()
					.insert(DBKey::from_slice(key), value);
			}
			None => {
				self.payload.borrow_mut().remove(&DBKey::from_slice(key));
			}
		}
		Ok(())
	}

	fn payload_drain_tx_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		unreachable!()
	}
	fn payload_apply(&self, _items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		unreachable!()
	}
	fn emit_event(&self, event: Event) -> VMResult<()> {
		self.events.borrow_mut().push(event);
		Ok(())
	}
	fn drain_tx_events(&self) -> VMResult<Vec<Event>> {
		Ok(self.events.borrow_mut().drain(..).collect())
	}
	fn apply_events(&self, _items: Vec<Event>) -> VMResult<()> {
		unreachable!()
	}
}

#[derive(Clone)]
struct StackedExecutorContext<EC: ExecutorContext> {
	executor_context: EC,
	payload_buffer_stack: Rc<VecDeque<Rc<RefCell<HashMap<DBKey, Option<DBValue>>>>>>,
	events_stack: Rc<VecDeque<Rc<RefCell<Vec<Event>>>>>,
}

impl<EC: ExecutorContext> StackedExecutorContext<EC> {
	fn new(executor_context: EC) -> Self {
		StackedExecutorContext {
			executor_context,
			payload_buffer_stack: {
				let mut stack = VecDeque::new();
				stack.push_back(Rc::new(RefCell::new(HashMap::new())));
				Rc::new(stack)
			},
			events_stack: {
				let mut stack = VecDeque::new();
				stack.push_back(Rc::new(RefCell::new(Vec::new())));
				Rc::new(stack)
			},
		}
	}
	fn derive(&self) -> Self {
		let executor_context = self.executor_context.clone();

		let mut payload_buffer_stack = (*self.payload_buffer_stack).clone();
		payload_buffer_stack.push_back(Rc::new(RefCell::new(HashMap::new())));
		let payload_buffer_stack = Rc::new(payload_buffer_stack);

		let mut events_stack = (*self.events_stack).clone();
		events_stack.push_back(Rc::new(RefCell::new(Vec::new())));
		let events_stack = Rc::new(events_stack);

		StackedExecutorContext {
			executor_context,
			payload_buffer_stack,
			events_stack,
		}
	}
}

impl<EC: ExecutorContext> ExecutorContext for StackedExecutorContext<EC> {
	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
		for buffer in self.payload_buffer_stack.iter().rev() {
			if let Some(value) = buffer.borrow().get(&DBKey::from_slice(key)) {
				return Ok(value.clone());
			}
		}
		self.executor_context.payload_get(key)
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		let buffer = self
			.payload_buffer_stack
			.back()
			.expect("payload_buffer_stack should not be empty");
		buffer.borrow_mut().insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_drain_tx_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		let buffer = self
			.payload_buffer_stack
			.back()
			.expect("payload_buffer_stack should not be empty");
		let buffer = buffer.borrow_mut().drain().collect();
		Ok(buffer)
	}
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		let second_last = if self.payload_buffer_stack.len() >= 2 {
			self.payload_buffer_stack
				.get(self.payload_buffer_stack.len() - 2)
		} else {
			None
		};
		match second_last {
			Some(second_last) => second_last.borrow_mut().extend(items),
			None => {
				let executor_context = &self.executor_context;
				for (k, v) in items {
					executor_context.payload_set(k.as_slice(), v)?;
				}
			}
		}
		Ok(())
	}
	fn emit_event(&self, event: Event) -> VMResult<()> {
		let events = self
			.events_stack
			.back()
			.expect("events_stack should not be empty");
		events.borrow_mut().push(event);
		Ok(())
	}
	fn drain_tx_events(&self) -> VMResult<Vec<Event>> {
		let events = self
			.events_stack
			.back()
			.expect("events_stack should not be empty");
		let events = events.borrow_mut().drain(..).collect();
		Ok(events)
	}
	fn apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		let second_last = if self.events_stack.len() >= 2 {
			self.events_stack.get(self.events_stack.len() - 2)
		} else {
			None
		};
		match second_last {
			Some(second_last) => second_last.borrow_mut().extend(items),
			None => {
				let executor_context = &self.executor_context;
				for item in items {
					executor_context.emit_event(item)?;
				}
			}
		}
		Ok(())
	}
}

pub struct TestVMContext<EC: ExecutorContext> {
	env: Rc<VMContextEnv>,
	call_env: Rc<VMCallEnv>,
	contract_env: Rc<VMContractEnv>,
	base_context: StackedExecutorContext<EC>,
	hash: Arc<HashImpl>,
	address: Arc<AddressImpl>,
	module_context: StackedExecutorContext<EC>,
}

impl<EC: ExecutorContext> TestVMContext<EC> {
	pub fn new(sender_address: Address, executor_context: EC) -> Self {
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
		let base_context = StackedExecutorContext::new(executor_context);
		let module_context = base_context.derive();
		let context = TestVMContext {
			env: Rc::new(VMContextEnv {
				number: 10,
				timestamp: 12345,
			}),
			call_env: Rc::new(VMCallEnv { tx_hash }),
			contract_env: Rc::new(VMContractEnv {
				contract_address,
				sender_address,
			}),
			base_context,
			hash: hash.clone(),
			address: address.clone(),
			module_context,
		};

		context
	}

	fn vm_to_module_key(&self, key: &[u8]) -> VMResult<Vec<u8>> {
		let key = &[&self.contract_env.contract_address.0, SEPARATOR, key].concat();
		let key = self.hash(key)?;
		let key = [b"contract", SEPARATOR, b"contract_data", SEPARATOR, &key.0].concat();
		Ok(key)
	}
}

impl<EC: ExecutorContext> VMContext for TestVMContext<EC> {
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
		let key = &self.vm_to_module_key(key)?;
		self.base_context.payload_get(key)
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		let key = &self.vm_to_module_key(key)?;
		self.base_context.payload_set(key, value)
	}
	fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		self.base_context.payload_drain_tx_buffer()
	}
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		self.base_context.payload_apply(items)
	}
	fn emit_event(&self, event: Event) -> VMResult<()> {
		self.base_context.emit_event(event)
	}
	fn drain_events(&self) -> VMResult<Vec<Event>> {
		self.base_context.drain_tx_events()
	}
	fn apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		self.base_context.apply_events(items)
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
	fn module_balance_get(&self, address: &Address) -> VMResult<Balance> {
		let balance: Option<Balance> =
			storage_map_get(self.module_context.clone(), b"balance", b"balance", address)?;
		let balance = balance.unwrap_or(0);
		Ok(balance)
	}
	fn module_balance_transfer(
		&self,
		sender: &Address,
		recipient: &Address,
		value: Balance,
	) -> VMResult<()> {
		let mut sender_balance = self.module_balance_get(sender)?;
		let mut recipient_balance = self.module_balance_get(recipient)?;

		if sender_balance < value {
			return Err(ContractError::Transfer.into());
		}

		sender_balance -= value;
		recipient_balance += value;

		storage_map_set(
			self.module_context.clone(),
			b"balance",
			b"balance",
			sender,
			&sender_balance,
		)?;
		storage_map_set(
			self.module_context.clone(),
			b"balance",
			b"balance",
			recipient,
			&recipient_balance,
		)?;

		self.module_context.emit_event(Event::from_data(
			"Transferred".to_string(),
			Transferred {
				sender: sender.clone(),
				recipient: recipient.clone(),
				value,
			},
		)?)?;

		Ok(())
	}

	fn module_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		self.module_context.payload_drain_tx_buffer()
	}
	fn module_payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		self.module_context.payload_apply(items)
	}
	fn module_drain_events(&self) -> VMResult<Vec<Event>> {
		self.module_context.drain_tx_events()
	}
	fn module_apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		self.module_context.apply_events(items)
	}
}

#[allow(dead_code)]
pub fn storage_mint<EC: ExecutorContext>(
	executor_context: EC,
	items: Vec<(Address, Balance)>,
) -> VMResult<()> {
	for (address, balance) in items {
		storage_map_set(
			executor_context.clone(),
			b"balance",
			b"balance",
			&address,
			&balance,
		)?;
	}
	Ok(())
}

fn storage_map_get<K: Encode, V: Decode, EC: ExecutorContext>(
	executor_context: EC,
	module_name: &[u8],
	storage_name: &[u8],
	key: &K,
) -> VMResult<Option<V>> {
	let key = codec::encode(key)?;
	let key = &[module_name, SEPARATOR, storage_name, SEPARATOR, &key].concat();
	let value = executor_context.payload_get(key)?;
	let value = match value {
		Some(value) => {
			let value = codec::decode(&value[..])?;
			Ok(Some(value))
		}
		None => Ok(None),
	};

	value
}

fn storage_map_set<K: Encode, V: Encode, EC: ExecutorContext>(
	executor_context: EC,
	module_name: &[u8],
	storage_name: &[u8],
	key: &K,
	value: &V,
) -> VMResult<()> {
	let key = codec::encode(key)?;
	let key = &[module_name, SEPARATOR, storage_name, SEPARATOR, &key].concat();

	let value = codec::encode(value)?;
	executor_context.payload_set(key, Some(value))?;
	Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct Transferred {
	pub sender: Address,
	pub recipient: Address,
	pub value: Balance,
}
