use byteorder::ByteOrder;
use serde::Serialize;
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
		_ => (),
	}
}

fn call_hello() {
	let len = input_len();
	let buffer = vec![0u8; len as usize];
	input_read(buffer.as_ptr() as _);

	let input: String = serde_json::from_slice(&buffer).unwrap();
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
	let key = vec![0u8; len as usize];
	input_read(key.as_ptr() as _);

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

	#[derive(Serialize)]
	struct Value {
		value: Option<Vec<u8>>,
	}

	let value = Value { value };

	let output = serde_json::to_vec(&value).unwrap();
	output_write(output.len() as _, output.as_ptr() as _);
}
