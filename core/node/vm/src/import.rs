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

use wasmer_runtime::Memory;
use wasmer_runtime::{func, imports};
use wasmer_runtime_core::import::ImportObject;
use wasmer_runtime_core::vm::Ctx;

use crate::errors::{ApplicationError, BusinessError, VMError, VMResult};

pub struct State {
	pub memory: Memory,
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
		}
	};
	Ok(import_object)
}

fn method_read(ctx: &mut Ctx, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	let ptr = ptr as usize;
	memory.view()[ptr..(ptr + state.method.len())]
		.iter()
		.zip(state.method.iter())
		.for_each(|(cell, v)| cell.set(*v));
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
	let ptr = ptr as usize;
	memory.view()[ptr..(ptr + state.input.len())]
		.iter()
		.zip(state.input.iter())
		.for_each(|(cell, v)| cell.set(*v));
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
	let ptr = ptr as usize;
	let len = len as usize;
	let mut output = vec![0u8; len];
	for (i, cell) in memory.view()[ptr..(ptr + len)].iter().enumerate() {
		output[i] = cell.get();
	}
	state.output = Some(output);
	Ok(())
}

fn error_return(ctx: &mut Ctx, len: u64, ptr: u64) -> VMResult<()> {
	let state = get_state(ctx);
	let memory = &state.memory;
	let ptr = ptr as usize;
	let len = len as usize;
	let mut error = vec![0u8; len];
	for (i, cell) in memory.view()[ptr..(ptr + len)].iter().enumerate() {
		error[i] = cell.get();
	}
	let msg = String::from_utf8(error).map_err(|_e| BusinessError::Deserialize)?;
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

fn get_state<'a>(ctx: &'a mut Ctx) -> &'a mut State {
	let state = unsafe { &mut *(ctx.data as *mut State) };
	state
}
