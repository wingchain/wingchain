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

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{DsaImpl, KeyPairImpl};
use crypto::hash::{Hash as HashT, HashImpl};
use node_vm::errors::{ContractError, VMResult};
use node_vm::{
	LazyCodeProvider, Mode, VMCallEnv, VMConfig, VMContext, VMContextEnv, VMContractEnv, VM,
};
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
	context: &dyn VMContext,
	mode: Mode,
	method: &str,
	params: &str,
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
	let code = LazyCodeProvider {
		code_hash,
		code: || Ok(Cow::Borrowed(code)),
	};
	let params = params.as_bytes();
	let result = vm.validate(&code, context, mode, method, params, pay_value)?;
	Ok(result)
}

pub fn vm_execute(
	code: &[u8],
	context: &dyn VMContext,
	mode: Mode,
	method: &str,
	params: &[u8],
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
	let code = LazyCodeProvider {
		code_hash,
		code: || Ok(Cow::Borrowed(code)),
	};
	let result = vm
		.execute(&code, context, mode, method, params, pay_value)
		.and_then(|result| {
			let result: String =
				String::from_utf8(result).map_err(|_| ContractError::Deserialize)?;
			Ok(result)
		});

	match result {
		Ok(result) => {
			context.payload_apply(context.payload_drain_buffer()?)?;
			context.apply_events(context.drain_events()?)?;
			Ok(result)
		}
		Err(e) => {
			context.payload_drain_buffer()?;
			context.drain_events()?;
			Err(e)
		}
	}
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
	config: Rc<VMConfig>,
	env: Rc<VMContextEnv>,
	call_env: Rc<VMCallEnv>,
	contract_env: Rc<VMContractEnv>,
	base_context: StackedExecutorContext<EC>,
	hash: Arc<HashImpl>,
	address: Arc<AddressImpl>,
	module_context: StackedExecutorContext<EC>,
	nested_vm_context: RefCell<Option<ClonableTestVMContext<EC>>>,
}

impl<EC: ExecutorContext> TestVMContext<EC> {
	pub fn new(
		config: VMConfig,
		tx_hash: Option<Hash>,
		contract_address: Option<Address>,
		sender_address: Option<Address>,
		context: EC,
	) -> Self {
		let base_context = StackedExecutorContext::new(context);
		Self::new_with_base_context(
			Rc::new(config),
			tx_hash,
			contract_address,
			sender_address,
			base_context,
		)
	}
	fn new_with_base_context(
		config: Rc<VMConfig>,
		tx_hash: Option<Hash>,
		contract_address: Option<Address>,
		sender_address: Option<Address>,
		base_context: StackedExecutorContext<EC>,
	) -> Self {
		let hash = Arc::new(HashImpl::Blake2b256);
		let address = Arc::new(AddressImpl::Blake2b160);

		let module_context = base_context.derive();

		let nested_vm_context = RefCell::new(None);

		let context = TestVMContext {
			config,
			env: Rc::new(VMContextEnv {
				number: 10,
				timestamp: 12345,
			}),
			call_env: Rc::new(VMCallEnv { tx_hash: tx_hash }),
			contract_env: Rc::new(VMContractEnv {
				contract_address,
				sender_address,
			}),
			base_context,
			hash: hash.clone(),
			address: address.clone(),
			module_context,
			nested_vm_context,
		};

		context
	}

	fn vm_to_module_key(&self, key: &[u8]) -> VMResult<Vec<u8>> {
		let contract_address = &self.contract_env.contract_address;
		let contract_address = contract_address
			.as_ref()
			.ok_or(ContractError::ContractAddressNotFound)?;
		let key = &[&contract_address.0, SEPARATOR, key].concat();
		let key = self.hash(key)?;
		let key = [b"contract", SEPARATOR, b"contract_data", SEPARATOR, &key.0].concat();
		Ok(key)
	}

	fn inner_get_code(
		&self,
		contract_address: &Address,
		version: Option<u32>,
	) -> VMResult<Option<Vec<u8>>> {
		let version = match version {
			Some(version) => version,
			None => {
				let current_version: Option<u32> = storage_map_get(
					self.module_context.clone(),
					b"contract",
					b"version",
					&contract_address,
				)?;
				match current_version {
					Some(current_version) => current_version,
					None => return Ok(None),
				}
			}
		};

		let key = codec::encode(&(&contract_address, version))?;
		let code = storage_map_raw_get(self.module_context.clone(), b"contract", b"code", &key)?;
		Ok(code)
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
			return Err(ContractError::Transfer {
				msg: "Insufficient balance".to_string(),
			}
			.into());
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
	fn nested_vm_contract_execute(
		&self,
		contract_address: &Address,
		method: &str,
		params: &[u8],
		pay_value: Balance,
	) -> VMResult<Vec<u8>> {
		if self.nested_vm_context.borrow().is_none() {
			let next_nest_depth = self.base_context.payload_buffer_stack.len() as u32 + 1;
			if next_nest_depth > self.config.max_nest_depth {
				return Err(ContractError::NestDepthExceeded.into());
			}
			let base_context = self.base_context.derive();
			let vm_context = ClonableTestVMContext {
				inner: Rc::new(TestVMContext::new_with_base_context(
					self.config.clone(),
					self.call_env.tx_hash.clone(),
					Some(contract_address.clone()),
					self.contract_env.contract_address.clone(),
					base_context,
				)),
			};
			self.nested_vm_context.borrow_mut().replace(vm_context);
		}

		let nested_vm_context = self
			.nested_vm_context
			.borrow()
			.as_ref()
			.expect("qed")
			.clone();

		let code = self
			.inner_get_code(&contract_address, None)?
			.ok_or(ContractError::NestedContractNotFound)?;
		let code_hash = self.hash(&code)?;
		let vm = VM::new((*self.config).clone());
		let code = LazyCodeProvider {
			code_hash,
			code: || Ok(Cow::Borrowed(&code)),
		};
		let result = vm.execute(
			&code,
			&nested_vm_context,
			Mode::Call,
			method,
			params,
			pay_value,
		)?;
		Ok(result)
	}
	fn nested_vm_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		let nested_vm_context = self
			.nested_vm_context
			.borrow()
			.as_ref()
			.expect("qed")
			.clone();
		nested_vm_context.payload_drain_buffer()
	}
	fn nested_vm_payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		let nested_vm_context = self
			.nested_vm_context
			.borrow()
			.as_ref()
			.expect("qed")
			.clone();
		nested_vm_context.payload_apply(items)
	}
	fn nested_vm_drain_events(&self) -> VMResult<Vec<Event>> {
		let nested_vm_context = self
			.nested_vm_context
			.borrow()
			.as_ref()
			.expect("qed")
			.clone();
		nested_vm_context.drain_events()
	}
	fn nested_vm_apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		let nested_vm_context = self
			.nested_vm_context
			.borrow()
			.as_ref()
			.expect("qed")
			.clone();
		nested_vm_context.apply_events(items)
	}
}

#[derive(Clone)]
struct ClonableTestVMContext<EC: ExecutorContext> {
	inner: Rc<TestVMContext<EC>>,
}

impl<EC: ExecutorContext> VMContext for ClonableTestVMContext<EC> {
	fn env(&self) -> Rc<VMContextEnv> {
		self.inner.env()
	}
	fn call_env(&self) -> Rc<VMCallEnv> {
		self.inner.call_env()
	}
	fn contract_env(&self) -> Rc<VMContractEnv> {
		self.inner.contract_env()
	}
	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
		self.inner.payload_get(key)
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		self.inner.payload_set(key, value)
	}
	fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		self.inner.payload_drain_buffer()
	}
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		self.inner.payload_apply(items)
	}
	fn emit_event(&self, event: Event) -> VMResult<()> {
		self.inner.emit_event(event)
	}
	fn drain_events(&self) -> VMResult<Vec<Event>> {
		self.inner.drain_events()
	}
	fn apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		self.inner.apply_events(items)
	}
	fn hash(&self, data: &[u8]) -> VMResult<Hash> {
		self.inner.hash(data)
	}
	fn address(&self, data: &[u8]) -> VMResult<Address> {
		self.inner.address(data)
	}
	fn validate_address(&self, address: &Address) -> VMResult<()> {
		self.inner.validate_address(address)
	}
	fn module_balance_get(&self, address: &Address) -> VMResult<Balance> {
		self.inner.module_balance_get(address)
	}
	fn module_balance_transfer(
		&self,
		sender: &Address,
		recipient: &Address,
		value: Balance,
	) -> VMResult<()> {
		self.inner.module_balance_transfer(sender, recipient, value)
	}
	fn module_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		self.inner.module_payload_drain_buffer()
	}
	fn module_payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		self.inner.module_payload_apply(items)
	}
	fn module_drain_events(&self) -> VMResult<Vec<Event>> {
		self.inner.module_drain_events()
	}
	fn module_apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		self.inner.module_apply_events(items)
	}
	fn nested_vm_contract_execute(
		&self,
		contract_address: &Address,
		method: &str,
		params: &[u8],
		pay_value: Balance,
	) -> VMResult<Vec<u8>> {
		self.inner
			.nested_vm_contract_execute(contract_address, method, params, pay_value)
	}
	fn nested_vm_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		self.inner.nested_vm_payload_drain_buffer()
	}
	fn nested_vm_payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		self.inner.nested_vm_payload_apply(items)
	}
	fn nested_vm_drain_events(&self) -> VMResult<Vec<Event>> {
		self.inner.nested_vm_drain_events()
	}
	fn nested_vm_apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		self.inner.nested_vm_apply_events(items)
	}
}

#[allow(dead_code)]
pub fn endow<EC: ExecutorContext>(
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

#[allow(dead_code)]
pub fn create_contract<EC: ExecutorContext>(
	contract_address: &Address,
	sender: &Address,
	code: &[u8],
	executor_context: EC,
	method: &str,
	params: &[u8],
	pay_value: Balance,
) -> VMResult<()> {
	let version = 1u32;
	storage_map_set(
		executor_context.clone(),
		b"contract",
		b"version",
		&contract_address,
		&version,
	)?;
	storage_map_set(
		executor_context.clone(),
		b"contract",
		b"code",
		&(&contract_address, version),
		&code,
	)?;
	let tx_hash = Some(Hash(vec![1]));

	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(contract_address.clone()),
		Some(sender.clone()),
		executor_context,
	);

	let _result = vm_execute(code, &context, Mode::Init, method, params, pay_value)?;

	Ok(())
}

fn storage_map_get<K: Encode, V: Decode, EC: ExecutorContext>(
	executor_context: EC,
	module_name: &[u8],
	storage_name: &[u8],
	key: &K,
) -> VMResult<Option<V>> {
	let key = codec::encode(key)?;
	storage_map_raw_get(executor_context, module_name, storage_name, &key)
}

fn storage_map_raw_get<V: Decode, EC: ExecutorContext>(
	executor_context: EC,
	module_name: &[u8],
	storage_name: &[u8],
	key: &[u8],
) -> VMResult<Option<V>> {
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
	storage_map_raw_set(executor_context, module_name, storage_name, &key, value)
}

fn storage_map_raw_set<V: Encode, EC: ExecutorContext>(
	executor_context: EC,
	module_name: &[u8],
	storage_name: &[u8],
	key: &[u8],
	value: &V,
) -> VMResult<()> {
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
