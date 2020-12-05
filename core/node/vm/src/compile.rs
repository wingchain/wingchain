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

use lru::LruCache;
use once_cell::sync::Lazy;
use parity_wasm::builder;
use parity_wasm::elements;
use parity_wasm::elements::{External, MemorySection, Type};
use parking_lot::Mutex;
use wasmer_runtime_core::wasmparser;
use wasmer_runtime_core::Module;

use primitives::Hash;

use crate::errors::{PreCompileError, VMResult};
use crate::{VMCodeProvider, VMConfig};

const CACHE_SIZE: usize = 1024;

static MODULE_CACHE: Lazy<Mutex<LruCache<Hash, Module>>> =
	Lazy::new(|| Mutex::new(LruCache::new(CACHE_SIZE)));

pub fn compile(code: &dyn VMCodeProvider, config: &VMConfig) -> VMResult<Module> {
	let code_hash = code.provide_code_hash()?;
	let mut guard = MODULE_CACHE.lock();
	if let Some(module) = guard.get(code_hash) {
		return Ok(module.clone());
	}

	let code = &*code.provide_code()?;
	let code = pre_compile(code, config)?;

	let module = wasmer_runtime::compile(&code)?;

	guard.put(code_hash.clone(), module.clone());

	Ok(module)
}

fn pre_compile(code: &[u8], config: &VMConfig) -> VMResult<Vec<u8>> {
	wasmparser::validate(code, None).map_err(|e| PreCompileError::ValidationError {
		msg: format!("{}", e),
	})?;

	let module: elements::Module =
		elements::deserialize_buffer(code).map_err(|e| PreCompileError::ValidationError {
			msg: format!("{}", e),
		})?;

	let module = import_memory(module, config)?;
	let module = validate_memory(module)?;
	let module = inject_stack_height_metering(module, config)?;
	let module = validate_imports(module, config)?;

	let code = elements::serialize(module).map_err(|_| PreCompileError::Serialize)?;

	Ok(code)
}

fn import_memory(mut module: elements::Module, config: &VMConfig) -> VMResult<elements::Module> {
	let mut tmp = MemorySection::default();

	module
		.memory_section_mut()
		.unwrap_or_else(|| &mut tmp)
		.entries_mut()
		.pop();

	let entry =
		elements::MemoryType::new(config.initial_memory_pages, Some(config.max_memory_pages));

	let mut builder = builder::from_module(module);
	builder.push_import(elements::ImportEntry::new(
		"env".to_string(),
		"memory".to_string(),
		elements::External::Memory(entry),
	));
	let module = builder.build();
	Ok(module)
}

fn validate_memory(module: elements::Module) -> VMResult<elements::Module> {
	if module
		.memory_section()
		.map_or(false, |ms| !ms.entries().is_empty())
	{
		Err(PreCompileError::InternalMemoryDeclared.into())
	} else {
		Ok(module)
	}
}

fn inject_stack_height_metering(
	module: elements::Module,
	config: &VMConfig,
) -> VMResult<elements::Module> {
	let module = pwasm_utils::stack_height::inject_limiter(module, config.max_stack_height)
		.map_err(|_| PreCompileError::StackHeightMetering)?;
	Ok(module)
}

fn validate_imports(module: elements::Module, config: &VMConfig) -> VMResult<elements::Module> {
	let types = module
		.type_section()
		.map(elements::TypeSection::types)
		.unwrap_or(&[]);
	let import_entries = module
		.import_section()
		.map(elements::ImportSection::entries)
		.unwrap_or(&[]);

	let mut imported_memory_type = None;

	for import in import_entries {
		if import.module() != "env" {
			return Err(PreCompileError::Imports.into());
		}

		let type_idx = match *import.external() {
			External::Function(ref type_idx) => type_idx,
			External::Memory(ref memory_type) => {
				imported_memory_type = Some(memory_type);
				continue;
			}
			_ => continue,
		};

		let Type::Function(ref _func_ty) = types
			.get(*type_idx as usize)
			.ok_or_else(|| PreCompileError::Imports)?;
	}
	if let Some(memory_type) = imported_memory_type {
		let limits = memory_type.limits();
		if limits.initial() != config.initial_memory_pages
			|| limits.maximum() != Some(config.max_memory_pages)
		{
			return Err(PreCompileError::Imports.into());
		}
	} else {
		return Err(PreCompileError::Imports.into());
	};

	Ok(module)
}
