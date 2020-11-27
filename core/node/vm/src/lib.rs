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

//! Virtual machine to execute contract

use std::collections::HashMap;
use std::rc::Rc;

use wasmer_runtime::wasm::MemoryDescriptor;
use wasmer_runtime::Memory;
use wasmer_runtime_core::units::Pages;

use primitives::{Address, Balance, BlockNumber, DBKey, DBValue, Event, Hash};

use crate::errors::{ErrorKind, VMError, VMResult};
use crate::import::State;

mod compile;
pub mod errors;
mod import;

pub struct VMConfig {
	max_stack_height: u32,
	initial_memory_pages: u32,
	max_memory_pages: u32,
	max_share_value_len: u64,
	max_share_size: u64,
}

impl Default for VMConfig {
	fn default() -> VMConfig {
		VMConfig {
			max_stack_height: 16 * 1024,
			initial_memory_pages: 2u32.pow(10),
			max_memory_pages: 2u32.pow(11),
			max_share_value_len: 2u64.pow(20) * 100,
			max_share_size: 1024,
		}
	}
}

pub struct VM {
	config: VMConfig,
}

pub enum Mode {
	Init,
	Call,
}

impl VM {
	pub fn new(config: VMConfig) -> Self {
		let vm = VM { config };
		vm
	}

	pub fn validate(
		&self,
		code_hash: &Hash,
		code: &[u8],
		mode: Mode,
		method: &str,
		params: &[u8],
		pay_value: Balance,
	) -> VMResult<()> {
		let func = match mode {
			Mode::Init => "validate_init",
			Mode::Call => "validate_call",
		};
		let context = &DummyVMContext;
		self.run(code_hash, code, context, func, method, params, pay_value)?;
		Ok(())
	}

	pub fn execute(
		&self,
		code_hash: &Hash,
		code: &[u8],
		context: &dyn VMContext,
		mode: Mode,
		method: &str,
		params: &[u8],
		pay_value: Balance,
	) -> VMResult<Vec<u8>> {
		let func = match mode {
			Mode::Init => "execute_init",
			Mode::Call => "execute_call",
		};
		self.run(code_hash, code, context, func, method, params, pay_value)
	}

	fn run(
		&self,
		code_hash: &Hash,
		code: &[u8],
		context: &dyn VMContext,
		func: &str,
		method: &str,
		params: &[u8],
		pay_value: Balance,
	) -> VMResult<Vec<u8>> {
		let module = compile::compile(code_hash, code, &self.config)?;

		let desc = MemoryDescriptor::new(
			Pages(self.config.initial_memory_pages),
			Some(Pages(self.config.max_memory_pages)),
			false,
		)
		.map_err(|e| VMError::System(ErrorKind::Wasm(e).into()))?;

		let memory = Memory::new(desc)?;
		let memory_copy = memory.clone();

		let mut state = State {
			config: &self.config,
			memory,
			shares: HashMap::new(),
			context,
			method,
			params,
			pay_value,
			result: None,
		};

		let import_object = import::import(&mut state, memory_copy)?;

		let instance = module.instantiate(&import_object)?;
		let _result = instance.call(func, &[])?;

		let output = state.result;
		let output = output.unwrap_or(serde_json::to_vec(&()).unwrap());
		Ok(output)
	}
}

pub trait VMContext {
	fn env(&self) -> Rc<VMContextEnv>;
	fn call_env(&self) -> Rc<VMCallEnv>;
	fn contract_env(&self) -> Rc<VMContractEnv>;
	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>>;
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()>;
	fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>>;
	fn payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()>;
	fn emit_event(&self, event: Event) -> VMResult<()>;
	fn drain_events(&self) -> VMResult<Vec<Event>>;
	fn apply_events(&self, items: Vec<Event>) -> VMResult<()>;
	fn hash(&self, data: &[u8]) -> VMResult<Hash>;
	fn address(&self, data: &[u8]) -> VMResult<Address>;
	fn validate_address(&self, address: &Address) -> VMResult<()>;
	fn module_balance_get(&self, address: &Address) -> VMResult<Balance>;
	fn module_balance_transfer(
		&self,
		sender: &Address,
		recipient: &Address,
		value: Balance,
	) -> VMResult<()>;
	fn module_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>>;
	fn module_payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()>;
	fn module_drain_events(&self) -> VMResult<Vec<Event>>;
	fn module_apply_events(&self, items: Vec<Event>) -> VMResult<()>;
	fn nested_vm_contract_execute(
		&self,
		contract_address: &Address,
		method: &str,
		params: &[u8],
		pay_value: Balance,
	) -> VMResult<Vec<u8>>;
	fn nested_vm_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>>;
	fn nested_vm_payload_apply(&self, items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()>;
	fn nested_vm_drain_events(&self) -> VMResult<Vec<Event>>;
	fn nested_vm_apply_events(&self, items: Vec<Event>) -> VMResult<()>;
}

pub struct VMContextEnv {
	pub number: BlockNumber,
	pub timestamp: u64,
}

pub struct VMCallEnv {
	/// tx hash is none for read call
	pub tx_hash: Option<Hash>,
}

pub struct VMContractEnv {
	pub contract_address: Address,
	/// sender address is none for read call
	pub sender_address: Option<Address>,
}

struct DummyVMContext;

impl VMContext for DummyVMContext {
	fn env(&self) -> Rc<VMContextEnv> {
		unreachable!()
	}
	fn call_env(&self) -> Rc<VMCallEnv> {
		unreachable!()
	}
	fn contract_env(&self) -> Rc<VMContractEnv> {
		unreachable!()
	}
	fn payload_get(&self, _key: &[u8]) -> VMResult<Option<DBValue>> {
		unreachable!()
	}
	fn payload_set(&self, _key: &[u8], _value: Option<DBValue>) -> VMResult<()> {
		unreachable!()
	}
	fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		unreachable!()
	}
	fn payload_apply(&self, _items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		unreachable!()
	}
	fn emit_event(&self, _event: Event) -> VMResult<()> {
		unreachable!()
	}
	fn drain_events(&self) -> VMResult<Vec<Event>> {
		unreachable!()
	}
	fn apply_events(&self, _items: Vec<Event>) -> VMResult<()> {
		unreachable!()
	}
	fn hash(&self, _data: &[u8]) -> VMResult<Hash> {
		unreachable!()
	}
	fn address(&self, _data: &[u8]) -> VMResult<Address> {
		unreachable!()
	}
	fn validate_address(&self, _address: &Address) -> VMResult<()> {
		unreachable!()
	}
	fn module_balance_get(&self, _address: &Address) -> VMResult<Balance> {
		unreachable!()
	}
	fn module_balance_transfer(
		&self,
		_sender: &Address,
		_recipient: &Address,
		_value: Balance,
	) -> VMResult<()> {
		unreachable!()
	}
	fn module_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		unreachable!()
	}
	fn module_payload_apply(&self, _items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		unreachable!()
	}
	fn module_drain_events(&self) -> VMResult<Vec<Event>> {
		unreachable!()
	}
	fn module_apply_events(&self, _items: Vec<Event>) -> VMResult<()> {
		unreachable!()
	}
	fn nested_vm_contract_execute(
		&self,
		_contract_address: &Address,
		_method: &str,
		_params: &[u8],
		_pay_value: Balance,
	) -> VMResult<Vec<u8>> {
		unreachable!()
	}
	fn nested_vm_payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		unreachable!()
	}
	fn nested_vm_payload_apply(&self, _items: Vec<(DBKey, Option<DBValue>)>) -> VMResult<()> {
		unreachable!()
	}
	fn nested_vm_drain_events(&self) -> VMResult<Vec<Event>> {
		unreachable!()
	}
	fn nested_vm_apply_events(&self, _items: Vec<Event>) -> VMResult<()> {
		unreachable!()
	}
}
