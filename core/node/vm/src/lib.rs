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

use std::rc::Rc;

use wasmer_runtime::wasm::MemoryDescriptor;
use wasmer_runtime::Memory;
use wasmer_runtime_core::units::Pages;

use primitives::codec::Encode;
use primitives::errors::CommonResult;
use primitives::{Address, BlockNumber, DBValue, Hash};

use crate::errors::{ErrorKind, VMError, VMResult};
use crate::import::State;

mod compile;
pub mod errors;
mod import;

pub struct VMConfig {
	max_stack_height: u32,
	initial_memory_pages: u32,
	max_memory_pages: u32,
}

impl Default for VMConfig {
	fn default() -> VMConfig {
		VMConfig {
			max_stack_height: 16 * 1024,
			initial_memory_pages: 2u32.pow(10),
			max_memory_pages: 2u32.pow(11),
		}
	}
}

pub struct VM {
	config: VMConfig,
	context: Rc<dyn VMContext>,
}

impl VM {
	pub fn new(config: VMConfig, context: Rc<dyn VMContext>) -> CommonResult<Self> {
		let vm = VM { config, context };
		Ok(vm)
	}

	pub fn execute(
		&self,
		code_hash: &Hash,
		code: &[u8],
		method: Vec<u8>,
		input: Vec<u8>,
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
			memory,
			context: self.context.clone(),
			method,
			input,
			output: None,
		};

		let import_object = import::import(&mut state, memory_copy)?;

		let instance = module.instantiate(&import_object)?;

		let _result = instance.call("execute_call", &[])?;

		let output = state.output;
		let output = output.unwrap_or(().encode());
		Ok(output)
	}
}

pub trait VMContext {
	fn env(&self) -> Rc<VMContextEnv>;
	fn call_env(&self) -> Rc<VMCallEnv>;
	fn storage_get(&self, key: &[u8]) -> VMResult<Option<DBValue>>;
	fn storage_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()>;
	fn emit_event(&self, event: Vec<u8>) -> VMResult<()>;
	fn hash(&self, data: &[u8]) -> VMResult<Hash>;
	fn address(&self, data: &[u8]) -> VMResult<Address>;
}

pub struct VMContextEnv {
	pub number: BlockNumber,
	pub timestamp: u64,
}

pub struct VMCallEnv {
	pub tx_hash: Hash,
}
