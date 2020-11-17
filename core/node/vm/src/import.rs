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

use std::ffi::c_void;
use std::rc::Rc;

use byteorder::ByteOrder;
use wasmer_runtime::Memory;
use wasmer_runtime::{func, imports};
use wasmer_runtime_core::import::ImportObject;
use wasmer_runtime_core::vm::Ctx;

use crate::errors::{ApplicationError, BusinessError, VMError, VMResult};
use crate::VMContext;

pub struct State {
	pub memory: Memory,
	pub context: Rc<dyn VMContext>,
	pub method: Vec<u8>,
	pub input: Vec<u8>,
	pub output: Option<Vec<u8>>,
}

struct StateRef(*mut c_void);

unsafe impl Send for StateRef {}

unsafe impl Sync for StateRef {}

pub fn import(state: &mut State, memory: Memory) -> VMResult<ImportObject> {
	let state_ref = StateRef(state as *mut _ as *mut c_void);

	let import_object = imports! {
		move || (state_ref.0, |_a| {}),
		"env" => {
			"memory" => memory,
			"method_read" => func!(method_read),
			"method_len" => func!(method_len),
			"input_read" => func!(input_read),
			"input_len" => func!(input_len),
			"output_write" => func!(output_write),
			"error_return" => func!(error_return),
			"abort" => func!(abort),
			"block_number" => func!(block_number),
			"block_timestamp" => func!(block_timestamp),
			"tx_hash_read" => func!(tx_hash_read),
			"tx_hash_len" => func!(tx_hash_len),
			"contract_address_read" => func!(contract_address_read),
			"contract_address_len" => func!(contract_address_len),
			"sender_address_read" => func!(sender_address_read),
			"sender_address_len" => func!(sender_address_len),
			"storage_read" => func!(storage_read),
			"storage_exist_len" => func!(storage_exist_len),
			"storage_write" => func!(storage_write),
			"event_write" => func!(event_write),
			"hash_read" => func!(hash_read),
			"hash_len" => func!(hash_len),
			"address_read" => func!(address_read),
			"address_len" => func!(address_len),
		}
	};
	Ok(import_object)
}

fn method_read(ctx: &mut Ctx, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	vec_to_memory(memory, ptr, &state.method);
	Ok(())
}

fn method_len(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	let result = state.method.len() as u64;
	Ok(result)
}

fn input_read(ctx: &mut Ctx, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	vec_to_memory(memory, ptr, &state.input);
	Ok(())
}

fn input_len(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	let result = state.input.len() as u64;
	Ok(result)
}

fn output_write(ctx: &mut Ctx, len: u64, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	let output = memory_to_vec(memory, len, ptr);
	state.output = Some(output);
	Ok(())
}

fn error_return(ctx: &mut Ctx, len: u64, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	let msg = memory_to_vec(memory, len, ptr);
	let msg = String::from_utf8(msg).map_err(|_e| BusinessError::Deserialize)?;
	let error = BusinessError::User { msg };
	Err(error.into())
}

/// for AssemblyScript
fn abort(_ctx: &mut Ctx, _msg_ptr: u32, _filename_ptr: u32, _line: u32, _col: u32) -> VMResult<()> {
	Err(VMError::Application(ApplicationError::BusinessError(
		BusinessError::Panic {
			msg: "AssemblyScript panic".to_string(),
		},
	)))
}

fn block_number(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	Ok(state.context.env().number)
}

fn block_timestamp(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	Ok(state.context.env().timestamp)
}

fn tx_hash_read(ctx: &mut Ctx, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	let tx_hash = &state.context.call_env().tx_hash.0[..];
	vec_to_memory(memory, ptr, tx_hash);
	Ok(())
}

fn tx_hash_len(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	let len = state.context.call_env().tx_hash.0.len() as u64;
	Ok(len)
}

fn contract_address_read(ctx: &mut Ctx, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	let contract_address = &state.context.call_env().contract_address.0[..];
	vec_to_memory(memory, ptr, contract_address);
	Ok(())
}

fn contract_address_len(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	let len = state.context.call_env().contract_address.0.len() as u64;
	Ok(len)
}

fn sender_address_read(ctx: &mut Ctx, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	let sender_address = &state.context.call_env().sender_address.0[..];
	vec_to_memory(memory, ptr, sender_address);
	Ok(())
}

fn sender_address_len(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	let len = state.context.call_env().sender_address.0.len() as u64;
	Ok(len)
}

fn storage_read(ctx: &mut Ctx, key_len: u64, key_ptr: u64, result_ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;

	let key = memory_to_vec(memory, key_len, key_ptr);

	let value = state.context.storage_get(&key)?;
	let value = value.ok_or(BusinessError::IllegalRead)?;

	vec_to_memory(memory, result_ptr, &value);
	Ok(())
}

fn storage_exist_len(ctx: &mut Ctx, key_len: u64, key_ptr: u64) -> VMResult<u64> {
	let state = get_state(ctx);
	let memory = &state.memory;

	let key = memory_to_vec(memory, key_len, key_ptr);

	let value = state.context.storage_get(&key)?;
	let (exist, len) = match value {
		Some(value) => (1u32, value.len() as u32),
		None => (0, 0),
	};
	let mut buffer = vec![0u8; 8];
	byteorder::LittleEndian::write_u32_into(&[exist, len], &mut buffer);
	let exist_len = byteorder::LittleEndian::read_u64(&buffer);

	Ok(exist_len)
}

fn storage_write(
	ctx: &mut Ctx,
	key_len: u64,
	key_ptr: u64,
	value_exist: u64,
	value_len: u64,
	value_ptr: u64,
) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;

	let key = memory_to_vec(memory, key_len, key_ptr);

	let value = match value_exist {
		1 => {
			let value = memory_to_vec(memory, value_len, value_ptr);
			Some(value)
		}
		_ => None,
	};

	state.context.storage_set(&key, value)?;

	Ok(())
}

fn event_write(ctx: &mut Ctx, len: u64, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;

	let event = memory_to_vec(memory, len, ptr);
	state.context.emit_event(event)?;
	Ok(())
}

fn hash_read(ctx: &mut Ctx, data_len: u64, data_ptr: u64, result_ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;

	let data = memory_to_vec(memory, data_len, data_ptr);

	let result = state.context.hash(&data)?.0;

	vec_to_memory(memory, result_ptr, &result);
	Ok(())
}

fn hash_len(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	let len = state.context.hash_len()?;
	Ok(len as u64)
}

fn address_read(ctx: &mut Ctx, data_len: u64, data_ptr: u64, result_ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;

	let data = memory_to_vec(memory, data_len, data_ptr);

	let result = state.context.address(&data)?.0;

	vec_to_memory(memory, result_ptr, &result);
	Ok(())
}

fn address_len(ctx: &mut Ctx) -> VMResult<u64> {
	let state = get_state(ctx);
	let len = state.context.address_len()?;
	Ok(len as u64)
}

fn memory_to_vec(memory: &Memory, len: u64, ptr: u64) -> Vec<u8> {
	let ptr = ptr as usize;
	let len = len as usize;
	let mut data = vec![0u8; len];
	for (i, cell) in memory.view()[ptr..(ptr + len)].iter().enumerate() {
		data[i] = cell.get();
	}
	data
}

fn vec_to_memory(memory: &Memory, ptr: u64, data: &[u8]) {
	let ptr = ptr as usize;
	memory.view()[ptr..(ptr + data.len())]
		.iter()
		.zip(data.iter())
		.for_each(|(cell, v)| cell.set(*v));
}

fn get_state(ctx: &mut Ctx) -> &mut State {
	let state = unsafe { &mut *(ctx.data as *mut State) };
	state
}
