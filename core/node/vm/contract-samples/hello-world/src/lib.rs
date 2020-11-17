use byteorder::ByteOrder;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

mod env {
	extern "C" {
		pub fn method_read(ptr: u64);
		pub fn method_len() -> u64;
		pub fn input_read(ptr: u64);
		pub fn input_len() -> u64;
		pub fn output_write(len: u64, ptr: u64);
		pub fn error_return(len: u64, ptr: u64);
		pub fn block_number() -> u64;
		pub fn block_timestamp() -> u64;
		pub fn tx_hash_read(ptr: u64);
		pub fn tx_hash_len() -> u64;
		pub fn storage_read(key_len: u64, key_ptr: u64, result_ptr: u64);
		pub fn storage_exist_len(key_len: u64, key_ptr: u64) -> u64;
		pub fn storage_write(
			key_len: u64,
			key_ptr: u64,
			value_exist: u64,
			value_len: u64,
			value_ptr: u64,
		);
		pub fn event_write(len: u64, ptr: u64);
		pub fn hash_read(data_len: u64, data_ptr: u64, result_ptr: u64);
		pub fn hash_len() -> u64;
	}
}

pub fn method_read(ptr: u64) {
	unsafe { env::method_read(ptr) }
}

pub fn method_len() -> u64 {
	unsafe { env::method_len() }
}

pub fn input_read(ptr: u64) {
	unsafe { env::input_read(ptr) }
}

pub fn input_len() -> u64 {
	unsafe { env::input_len() }
}

pub fn output_write(len: u64, ptr: u64) {
	unsafe { env::output_write(len, ptr) }
}

pub fn error_return(len: u64, ptr: u64) {
	unsafe { env::error_return(len, ptr) }
}

pub fn block_number() -> u64 {
	unsafe { env::block_number() }
}

pub fn block_timestamp() -> u64 {
	unsafe { env::block_timestamp() }
}

pub fn tx_hash_read(ptr: u64) {
	unsafe { env::tx_hash_read(ptr) }
}

pub fn tx_hash_len() -> u64 {
	unsafe { env::tx_hash_len() }
}

pub fn storage_read(key_len: u64, key_ptr: u64, result_ptr: u64) {
	unsafe { env::storage_read(key_len, key_ptr, result_ptr) }
}

pub fn storage_exist_len(key_len: u64, key_ptr: u64) -> u64 {
	unsafe { env::storage_exist_len(key_len, key_ptr) }
}

pub fn storage_write(key_len: u64, key_ptr: u64, value_exist: u64, value_len: u64, value_ptr: u64) {
	unsafe { env::storage_write(key_len, key_ptr, value_exist, value_len, value_ptr) }
}

pub fn event_write(len: u64, ptr: u64) {
	unsafe { env::event_write(len, ptr) }
}

pub fn hash_read(data_len: u64, data_ptr: u64, result_ptr: u64) {
	unsafe { env::hash_read(data_len, data_ptr, result_ptr) }
}

pub fn hash_len() -> u64 {
	unsafe { env::hash_len() }
}

#[wasm_bindgen]
pub fn execute_call() {
	let len = method_len();
	let method = vec![0u8; len as usize];
	method_read(method.as_ptr() as _);

	let method = String::from_utf8(method).unwrap();
	let method = method.as_str();

	match method {
		"hello" => call_hello(),
		"error" => call_error(),
		"block_number" => call_block_number(),
		"block_timestamp" => call_block_timestamp(),
		"tx_hash" => call_tx_hash(),
		"storage_get" => call_storage_get(),
		"storage_set" => call_storage_set(),
		"event" => call_event(),
		"hash" => call_hash(),
		_ => (),
	}
}

fn call_hello() {
	let len = input_len();
	let input = vec![0u8; len as usize];
	input_read(input.as_ptr() as _);

	let input: String = serde_json::from_slice(&input).unwrap();
	let output = format!("hello {}", input);
	let output = serde_json::to_vec(&output).unwrap();

	output_write(output.len() as _, output.as_ptr() as _);
}

fn call_error() {
	let error = "custom error";
	error_return(error.len() as _, error.as_ptr() as _);
}

fn call_block_number() {
	let number = block_number();
	let output = serde_json::to_vec(&number).unwrap();
	output_write(output.len() as _, output.as_ptr() as _);
}

fn call_block_timestamp() {
	let timestamp = block_timestamp();
	let output = serde_json::to_vec(&timestamp).unwrap();
	output_write(output.len() as _, output.as_ptr() as _);
}

fn call_tx_hash() {
	let len = tx_hash_len();
	let tx_hash = vec![0u8; len as usize];
	tx_hash_read(tx_hash.as_ptr() as _);

	let output = serde_json::to_vec(&tx_hash).unwrap();
	output_write(output.len() as _, output.as_ptr() as _);
}

fn call_storage_get() {
	let len = input_len();
	let input = vec![0u8; len as usize];
	input_read(input.as_ptr() as _);

	let key: Vec<u8> = serde_json::from_slice(&input).unwrap();

	let exist_len = storage_exist_len(key.len() as _, key.as_ptr() as _);

	let mut buffer = vec![0u8; 8];
	byteorder::LittleEndian::write_u64(&mut buffer, exist_len);
	let mut exist_len = [0; 2];
	byteorder::LittleEndian::read_u32_into(&buffer, &mut exist_len);
	let (exist, len) = (exist_len[0], exist_len[1]);

	let value = match exist {
		1 => {
			let value = vec![0u8; len as usize];
			storage_read(key.len() as _, key.as_ptr() as _, value.as_ptr() as _);
			Some(value)
		}
		_ => None,
	};

	let output = serde_json::to_vec(&value).unwrap();
	output_write(output.len() as _, output.as_ptr() as _);
}

fn call_storage_set() {
	let len = input_len();
	let input = vec![0u8; len as usize];
	input_read(input.as_ptr() as _);

	#[derive(Deserialize)]
	struct Input {
		key: Vec<u8>,
		value: Option<Vec<u8>>,
	}
	let input: Input = serde_json::from_slice(&input).unwrap();

	match input.value {
		Some(value) => {
			storage_write(
				input.key.len() as _,
				input.key.as_ptr() as _,
				1,
				value.len() as _,
				value.as_ptr() as _,
			);
		}
		None => {
			storage_write(input.key.len() as _, input.key.as_ptr() as _, 0, 0, 0);
		}
	}
}

fn call_event() {
	#[derive(Serialize)]
	struct Event {
		name: String,
	}
	let event = Event {
		name: "MyEvent".to_string(),
	};
	let event = serde_json::to_vec(&event).unwrap();

	event_write(event.len() as _, event.as_ptr() as _);
}

fn call_hash() {
	let len = input_len();
	let input = vec![0u8; len as usize];
	input_read(input.as_ptr() as _);
	let input: Vec<u8> = serde_json::from_slice(&input).unwrap();

	let hash_len = hash_len();
	let hash = vec![0u8; hash_len as usize];

	hash_read(input.len() as _, input.as_ptr() as _, hash.as_ptr() as _);

	let output = hash;
	let output = serde_json::to_vec(&output).unwrap();

	output_write(output.len() as _, output.as_ptr() as _);
}