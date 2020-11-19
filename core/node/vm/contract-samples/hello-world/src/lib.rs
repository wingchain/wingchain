use byteorder::ByteOrder;
use sdk::import;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn execute_call() {
	let len = import::method_len();
	let method = vec![0u8; len as usize];
	import::method_read(method.as_ptr() as _);

	let method = String::from_utf8(method).unwrap();
	let method = method.as_str();

	match method {
		"hello" => call_hello(),
		"error" => call_error(),
		"block_number" => call_env_block_number(),
		"block_timestamp" => call_env_block_timestamp(),
		"tx_hash" => call_env_tx_hash(),
		"contract_address" => call_env_contract_address(),
		"sender_address" => call_env_sender_address(),
		"pay_value" => call_env_pay_value(),
		"storage_get" => call_storage_get(),
		"storage_set" => call_storage_set(),
		"event" => call_event(),
		"compute_hash" => call_compute_hash(),
		"compute_address" => call_compute_address(),
		"balance" => call_balance(),
		"balance_transfer" => call_balance_transfer(),
		_ => (),
	}
}

fn call_hello() {
	let len = import::input_len();
	let input = vec![0u8; len as usize];
	import::input_read(input.as_ptr() as _);

	let input: String = serde_json::from_slice(&input).unwrap();
	let output = format!("hello {}", input);
	let output = serde_json::to_vec(&output).unwrap();

	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_error() {
	let error = "custom error";
	import::error_return(error.len() as _, error.as_ptr() as _);
}

fn call_env_block_number() {
	let number = import::env_block_number();
	let output = serde_json::to_vec(&number).unwrap();
	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_env_block_timestamp() {
	let timestamp = import::env_block_timestamp();
	let output = serde_json::to_vec(&timestamp).unwrap();
	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_env_tx_hash() {
	let len = import::env_tx_hash_len();
	let tx_hash = vec![0u8; len as usize];
	import::env_tx_hash_read(tx_hash.as_ptr() as _);

	let output = serde_json::to_vec(&tx_hash).unwrap();
	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_env_contract_address() {
	let len = import::env_contract_address_len();
	let contract_address = vec![0u8; len as usize];
	import::env_contract_address_read(contract_address.as_ptr() as _);

	let output = serde_json::to_vec(&contract_address).unwrap();
	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_env_sender_address() {
	let len = import::env_sender_address_len();
	let sender_address = vec![0u8; len as usize];
	import::env_sender_address_read(sender_address.as_ptr() as _);

	let output = serde_json::to_vec(&sender_address).unwrap();
	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_env_pay_value() {
	let pay_value = import::env_pay_value();
	let output = serde_json::to_vec(&pay_value).unwrap();
	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_storage_get() {
	let len = import::input_len();
	let input = vec![0u8; len as usize];
	import::input_read(input.as_ptr() as _);

	let key: Vec<u8> = serde_json::from_slice(&input).unwrap();

	let exist_len = import::storage_exist_len(key.len() as _, key.as_ptr() as _);

	let mut buffer = vec![0u8; 8];
	byteorder::LittleEndian::write_u64(&mut buffer, exist_len);
	let mut exist_len = [0; 2];
	byteorder::LittleEndian::read_u32_into(&buffer, &mut exist_len);
	let (exist, len) = (exist_len[0], exist_len[1]);

	let value = match exist {
		1 => {
			let value = vec![0u8; len as usize];
			import::storage_read(key.len() as _, key.as_ptr() as _, value.as_ptr() as _);
			Some(value)
		}
		_ => None,
	};

	let output = serde_json::to_vec(&value).unwrap();
	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_storage_set() {
	let len = import::input_len();
	let input = vec![0u8; len as usize];
	import::input_read(input.as_ptr() as _);

	#[derive(Deserialize)]
	struct Input {
		key: Vec<u8>,
		value: Option<Vec<u8>>,
	}
	let input: Input = serde_json::from_slice(&input).unwrap();

	match input.value {
		Some(value) => {
			import::storage_write(
				input.key.len() as _,
				input.key.as_ptr() as _,
				1,
				value.len() as _,
				value.as_ptr() as _,
			);
		}
		None => {
			import::storage_write(input.key.len() as _, input.key.as_ptr() as _, 0, 0, 0);
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

	import::event_write(event.len() as _, event.as_ptr() as _);
}

fn call_compute_hash() {
	let len = import::input_len();
	let input = vec![0u8; len as usize];
	import::input_read(input.as_ptr() as _);
	let input: Vec<u8> = serde_json::from_slice(&input).unwrap();

	let hash_len = import::compute_hash_len();
	let hash = vec![0u8; hash_len as usize];

	import::compute_hash(input.len() as _, input.as_ptr() as _, hash.as_ptr() as _);

	let output = hash;
	let output = serde_json::to_vec(&output).unwrap();

	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_compute_address() {
	let len = import::input_len();
	let input = vec![0u8; len as usize];
	import::input_read(input.as_ptr() as _);
	let input: Vec<u8> = serde_json::from_slice(&input).unwrap();

	let address_len = import::compute_address_len();
	let address = vec![0u8; address_len as usize];

	import::compute_address(input.len() as _, input.as_ptr() as _, address.as_ptr() as _);
	let output = address;
	let output = serde_json::to_vec(&output).unwrap();

	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_balance() {
	let len = import::input_len();
	let input = vec![0u8; len as usize];
	import::input_read(input.as_ptr() as _);
	let input: Vec<u8> = serde_json::from_slice(&input).unwrap();

	let address = input;

	let balance = import::balance_read(address.len() as _, address.as_ptr() as _);

	let output = balance;
	let output = serde_json::to_vec(&output).unwrap();

	import::output_write(output.len() as _, output.as_ptr() as _);
}

fn call_balance_transfer() {
	let len = import::input_len();
	let input = vec![0u8; len as usize];
	import::input_read(input.as_ptr() as _);

	#[derive(Deserialize)]
	struct Input {
		recipient: Vec<u8>,
		value: u64,
	}
	let input: Input = serde_json::from_slice(&input).unwrap();

	import::balance_transfer(
		input.recipient.len() as _,
		input.recipient.as_ptr() as _,
		input.value,
	);
}
