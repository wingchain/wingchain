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

use executor_primitives::{
	self, CallEnv, Context, ContextEnv, EmptyParams, Module, ModuleError, ModuleResult, StorageMap,
	Util, SEPARATOR,
};
use module_balance::TransferParams;
use node_vm::errors::{ContractError, VMError, VMResult};
use node_vm::{Mode, VMCallEnv, VMConfig, VMContext, VMContextEnv, VMContractEnv, VM};
use primitives::{codec, Address, Balance, DBKey, DBValue, Event, Hash};

const CONTRACT_DATA_STORAGE_KEY: &[u8] = b"contract_data";

#[derive(Clone)]
struct StackedExecutorContext<EC: Context> {
	executor_context: EC,
	meta_buffer_stack: Rc<VecDeque<Rc<RefCell<HashMap<DBKey, Option<DBValue>>>>>>,
	payload_buffer_stack: Rc<VecDeque<Rc<RefCell<HashMap<DBKey, Option<DBValue>>>>>>,
	events_stack: Rc<VecDeque<Rc<RefCell<Vec<Event>>>>>,
}

impl<EC: Context> StackedExecutorContext<EC> {
	fn new(executor_context: EC) -> Self {
		StackedExecutorContext {
			executor_context,
			meta_buffer_stack: {
				let mut stack = VecDeque::new();
				stack.push_back(Rc::new(RefCell::new(HashMap::new())));
				Rc::new(stack)
			},
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

		let mut meta_buffer_stack = (*self.meta_buffer_stack).clone();
		meta_buffer_stack.push_back(Rc::new(RefCell::new(HashMap::new())));
		let meta_buffer_stack = Rc::new(meta_buffer_stack);

		let mut payload_buffer_stack = (*self.payload_buffer_stack).clone();
		payload_buffer_stack.push_back(Rc::new(RefCell::new(HashMap::new())));
		let payload_buffer_stack = Rc::new(payload_buffer_stack);

		let mut events_stack = (*self.events_stack).clone();
		events_stack.push_back(Rc::new(RefCell::new(Vec::new())));
		let events_stack = Rc::new(events_stack);

		StackedExecutorContext {
			executor_context,
			meta_buffer_stack,
			payload_buffer_stack,
			events_stack,
		}
	}
}

impl<EC: Context> Context for StackedExecutorContext<EC> {
	fn env(&self) -> Rc<ContextEnv> {
		self.executor_context.env()
	}
	fn call_env(&self) -> Rc<CallEnv> {
		self.executor_context.call_env()
	}
	fn meta_get(&self, key: &[u8]) -> ModuleResult<Option<DBValue>> {
		for buffer in self.meta_buffer_stack.iter().rev() {
			if let Some(value) = buffer.borrow().get(&DBKey::from_slice(key)) {
				return Ok(value.clone());
			}
		}
		self.executor_context.meta_get(key)
	}
	fn meta_set(&self, key: &[u8], value: Option<DBValue>) -> ModuleResult<()> {
		let buffer = self
			.meta_buffer_stack
			.back()
			.expect("meta_buffer_stack should not be empty");
		buffer.borrow_mut().insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn meta_drain_tx_buffer(&self) -> ModuleResult<Vec<(DBKey, Option<DBValue>)>> {
		let buffer = self
			.meta_buffer_stack
			.back()
			.expect("meta_buffer_stack should not be empty");
		let buffer = buffer.borrow_mut().drain().collect();
		Ok(buffer)
	}
	fn meta_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> ModuleResult<()> {
		let second_last = if self.payload_buffer_stack.len() >= 2 {
			self.meta_buffer_stack
				.get(self.payload_buffer_stack.len() - 2)
		} else {
			None
		};
		match second_last {
			Some(second_last) => second_last.borrow_mut().extend(items),
			None => {
				let executor_context = &self.executor_context;
				for (k, v) in items {
					executor_context.meta_set(k.as_slice(), v)?;
				}
			}
		}
		Ok(())
	}
	fn payload_get(&self, key: &[u8]) -> ModuleResult<Option<DBValue>> {
		for buffer in self.payload_buffer_stack.iter().rev() {
			if let Some(value) = buffer.borrow().get(&DBKey::from_slice(key)) {
				return Ok(value.clone());
			}
		}
		self.executor_context.payload_get(key)
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> ModuleResult<()> {
		let buffer = self
			.payload_buffer_stack
			.back()
			.expect("payload_buffer_stack should not be empty");
		buffer.borrow_mut().insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_drain_tx_buffer(&self) -> ModuleResult<Vec<(DBKey, Option<DBValue>)>> {
		let buffer = self
			.payload_buffer_stack
			.back()
			.expect("payload_buffer_stack should not be empty");
		let buffer = buffer.borrow_mut().drain().collect();
		Ok(buffer)
	}
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> ModuleResult<()> {
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
	fn emit_event(&self, event: Event) -> ModuleResult<()> {
		let events = self
			.events_stack
			.back()
			.expect("events_stack should not be empty");
		events.borrow_mut().push(event);
		Ok(())
	}
	fn drain_tx_events(&self) -> ModuleResult<Vec<Event>> {
		let events = self
			.events_stack
			.back()
			.expect("events_stack should not be empty");
		let events = events.borrow_mut().drain(..).collect();
		Ok(events)
	}
	fn apply_events(&self, items: Vec<Event>) -> ModuleResult<()> {
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

pub struct DefaultVMContext<M: Module> {
	config: Rc<VMConfig>,
	env: Rc<VMContextEnv>,
	call_env: Rc<VMCallEnv>,
	contract_env: Rc<VMContractEnv>,
	executor_util: M::U,
	base_context: StackedExecutorContext<M::C>,
	module_context: StackedExecutorContext<M::C>,
	nested_vm_context: RefCell<Option<ClonableDefaultVMContext<M>>>,
	version: StorageMap<Address, u32, M>,
	code: StorageMap<(Address, u32), Vec<u8>, M>,
}

impl<M: Module> DefaultVMContext<M> {
	pub fn new(
		config: VMConfig,
		contract_env: Rc<VMContractEnv>,
		executor_context: M::C,
		executor_util: M::U,
	) -> Self {
		let base_context = StackedExecutorContext::new(executor_context);
		Self::new_with_base_context(Rc::new(config), contract_env, base_context, executor_util)
	}
	fn new_with_base_context(
		config: Rc<VMConfig>,
		contract_env: Rc<VMContractEnv>,
		base_context: StackedExecutorContext<M::C>,
		executor_util: M::U,
	) -> Self {
		let env = {
			let env = base_context.env();
			Rc::new(VMContextEnv {
				number: env.number,
				timestamp: env.timestamp,
			})
		};
		let call_env = {
			let call_env = base_context.call_env();
			Rc::new(VMCallEnv {
				tx_hash: call_env.tx_hash.clone(),
			})
		};
		let module_context = base_context.derive();
		let nested_vm_context = RefCell::new(None);

		let version = StorageMap::new(base_context.executor_context.clone(), b"version");
		let code = StorageMap::new(base_context.executor_context.clone(), b"code");

		DefaultVMContext {
			config,
			env,
			call_env,
			contract_env,
			executor_util,
			base_context,
			module_context,
			nested_vm_context,
			version,
			code,
		}
	}

	/// Translate key in vm to key in module
	/// [module_key]_[contract_data_storage_key]_hash([contract_address]_[key])
	fn vm_to_module_key(&self, key: &[u8]) -> VMResult<Vec<u8>> {
		let key = &[&self.contract_env.contract_address.0, SEPARATOR, key].concat();
		let key = self
			.executor_util
			.hash(key)
			.map_err(Self::module_to_vm_error)?;
		let key = [
			M::STORAGE_KEY,
			SEPARATOR,
			CONTRACT_DATA_STORAGE_KEY,
			SEPARATOR,
			&key.0,
		]
		.concat();
		Ok(key)
	}

	fn module_to_vm_error(e: ModuleError) -> VMError {
		match e {
			ModuleError::System(e) => VMError::System(e),
			ModuleError::Application(e) => match e {
				executor_primitives::errors::ApplicationError::InvalidAddress(_) => {
					ContractError::InvalidAddress.into()
				}
				executor_primitives::errors::ApplicationError::Unsigned => {
					ContractError::Unsigned.into()
				}
				executor_primitives::errors::ApplicationError::User { msg } => {
					(ContractError::User { msg }).into()
				}
			},
		}
	}

	fn inner_get_code(
		&self,
		contract_address: &Address,
		version: Option<u32>,
	) -> VMResult<Option<Vec<u8>>> {
		let version = match version {
			Some(version) => version,
			None => {
				let current_version = self
					.version
					.get(&contract_address)
					.map_err(Self::module_to_vm_error)?;
				match current_version {
					Some(current_version) => current_version,
					None => return Ok(None),
				}
			}
		};

		let key = codec::encode(&(&contract_address, version))?;
		let code = self.code.raw_get(&key).map_err(Self::module_to_vm_error)?;
		Ok(code)
	}
}

impl<M: Module> VMContext for DefaultVMContext<M> {
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
		let result = self
			.base_context
			.payload_get(key)
			.map_err(Self::module_to_vm_error)?;
		Ok(result)
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		let key = &self.vm_to_module_key(key)?;
		let result = self
			.base_context
			.payload_set(key, value)
			.map_err(Self::module_to_vm_error)?;
		Ok(result)
	}
	fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		let result = self
			.base_context
			.payload_drain_tx_buffer()
			.map_err(Self::module_to_vm_error)?;
		Ok(result)
	}
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		self.base_context
			.payload_apply(items)
			.map_err(Self::module_to_vm_error)?;
		Ok(())
	}
	fn emit_event(&self, event: Event) -> VMResult<()> {
		self.base_context
			.emit_event(event)
			.map_err(Self::module_to_vm_error)?;
		Ok(())
	}
	fn drain_events(&self) -> VMResult<Vec<Event>> {
		let result = self
			.base_context
			.drain_tx_events()
			.map_err(Self::module_to_vm_error)?;
		Ok(result)
	}
	fn apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		self.base_context
			.apply_events(items)
			.map_err(Self::module_to_vm_error)?;
		Ok(())
	}
	fn hash(&self, data: &[u8]) -> VMResult<Hash> {
		self.executor_util
			.hash(data)
			.map_err(Self::module_to_vm_error)
	}
	fn address(&self, data: &[u8]) -> VMResult<Address> {
		self.executor_util
			.address(data)
			.map_err(Self::module_to_vm_error)
	}
	fn validate_address(&self, address: &Address) -> VMResult<()> {
		self.executor_util
			.validate_address(address)
			.map_err(Self::module_to_vm_error)
	}
	fn module_balance_get(&self, address: &Address) -> VMResult<Balance> {
		let balance_module =
			module_balance::Module::new(self.module_context.clone(), self.executor_util.clone());
		let sender = Some(address);
		balance_module
			.get_balance(sender, EmptyParams)
			.map_err(Self::module_to_vm_error)
	}
	fn module_balance_transfer(
		&self,
		sender: &Address,
		recipient: &Address,
		value: Balance,
	) -> VMResult<()> {
		let params = TransferParams {
			recipient: recipient.clone(),
			value,
		};
		module_balance::Module::<M::C, M::U>::validate_transfer(
			&self.executor_util,
			params.clone(),
		)
		.map_err(Self::module_to_vm_error)?;
		let balance_module =
			module_balance::Module::new(self.module_context.clone(), self.executor_util.clone());
		let sender = Some(sender);
		balance_module
			.transfer(sender, params)
			.map_err(Self::module_to_vm_error)?;
		Ok(())
	}
	fn module_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		let result = self
			.module_context
			.payload_drain_tx_buffer()
			.map_err(Self::module_to_vm_error)?;
		Ok(result)
	}
	fn module_payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		self.base_context
			.payload_apply(items)
			.map_err(Self::module_to_vm_error)?;
		Ok(())
	}
	fn module_drain_events(&self) -> VMResult<Vec<Event>> {
		let result = self
			.module_context
			.drain_tx_events()
			.map_err(Self::module_to_vm_error)?;
		Ok(result)
	}
	fn module_apply_events(&self, items: Vec<Event>) -> VMResult<()> {
		self.module_context
			.apply_events(items)
			.map_err(Self::module_to_vm_error)?;
		Ok(())
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

			// when contract A executes the nested contract B
			// the sender of contract B is contract A
			let contract_env = Rc::new(VMContractEnv {
				contract_address: contract_address.clone(),
				sender_address: Some(self.contract_env.contract_address.clone()),
			});

			let vm_context = ClonableDefaultVMContext {
				inner: Rc::new(DefaultVMContext::new_with_base_context(
					self.config.clone(),
					contract_env,
					base_context,
					self.executor_util.clone(),
				)),
			};
			self.nested_vm_context.borrow_mut().replace(vm_context);
		}

		let nested_vm_context = (*self.nested_vm_context.borrow().as_ref().expect("qed")).clone();

		let code = self
			.inner_get_code(&contract_address, None)?
			.ok_or(ContractError::NestedContractNotFound)?;
		let code_hash = self.hash(&code)?;
		let vm = VM::new((*self.config).clone());

		let result = vm.execute(
			&code_hash,
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

struct ClonableDefaultVMContext<M: Module> {
	inner: Rc<DefaultVMContext<M>>,
}

impl<M: Module> Clone for ClonableDefaultVMContext<M> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

impl<M: Module> VMContext for ClonableDefaultVMContext<M> {
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
