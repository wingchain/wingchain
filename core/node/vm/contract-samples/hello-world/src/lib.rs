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

#[wasm_bindgen]
pub fn execute_call() {
	let len = method_len();
	let method = vec![0u8; len as usize];
	method_read(method.as_ptr() as _);

	let method = String::from_utf8(method).unwrap();
	let method = method.as_str();

	match method {
		"hello" => hello(),
		"error" => error(),
		_ => (),
	}
}

fn hello() {
	let len = input_len();
	let buffer = vec![0u8; len as usize];
	input_read(buffer.as_ptr() as _);

	let input: String = serde_json::from_slice(&buffer).unwrap();
	let output = format!("hello {}", input);
	let output = serde_json::to_vec(&output).unwrap();

	output_write(output.len() as _, output.as_ptr() as _);
}

fn error() {
	let error = "custom error";
	error_return(error.len() as _, error.as_ptr() as _);
}
