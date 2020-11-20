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

mod env {
	extern "C" {
		pub fn method_read(ptr: u64);
		pub fn method_len() -> u64;
		pub fn input_read(ptr: u64);
		pub fn input_len() -> u64;
		pub fn output_write(len: u64, ptr: u64);
		pub fn error_return(len: u64, ptr: u64);
		pub fn env_block_number() -> u64;
		pub fn env_block_timestamp() -> u64;
		pub fn env_tx_hash_read(ptr: u64);
		pub fn env_tx_hash_len() -> u64;
		pub fn env_contract_address_read(ptr: u64);
		pub fn env_contract_address_len() -> u64;
		pub fn env_sender_address_read(ptr: u64);
		pub fn env_sender_address_len() -> u64;
		pub fn env_pay_value() -> u64;
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
		pub fn compute_hash(data_len: u64, data_ptr: u64, result_ptr: u64);
		pub fn compute_hash_len() -> u64;
		pub fn compute_address(data_len: u64, data_ptr: u64, result_ptr: u64);
		pub fn compute_address_len() -> u64;
		pub fn balance_read(address_len: u64, address_ptr: u64) -> u64;
		pub fn balance_transfer(recipient_address_len: u64, recipient_address_ptr: u64, value: u64);
		pub fn pay();
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

pub fn env_block_number() -> u64 {
	unsafe { env::env_block_number() }
}

pub fn env_block_timestamp() -> u64 {
	unsafe { env::env_block_timestamp() }
}

pub fn env_tx_hash_read(ptr: u64) {
	unsafe { env::env_tx_hash_read(ptr) }
}

pub fn env_tx_hash_len() -> u64 {
	unsafe { env::env_tx_hash_len() }
}

pub fn env_contract_address_read(ptr: u64) {
	unsafe { env::env_contract_address_read(ptr) }
}

pub fn env_contract_address_len() -> u64 {
	unsafe { env::env_contract_address_len() }
}

pub fn env_sender_address_read(ptr: u64) {
	unsafe { env::env_sender_address_read(ptr) }
}

pub fn env_sender_address_len() -> u64 {
	unsafe { env::env_sender_address_len() }
}

pub fn env_pay_value() -> u64 {
	unsafe { env::env_pay_value() }
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

pub fn compute_hash(data_len: u64, data_ptr: u64, result_ptr: u64) {
	unsafe { env::compute_hash(data_len, data_ptr, result_ptr) }
}

pub fn compute_hash_len() -> u64 {
	unsafe { env::compute_hash_len() }
}

pub fn compute_address(data_len: u64, data_ptr: u64, result_ptr: u64) {
	unsafe { env::compute_address(data_len, data_ptr, result_ptr) }
}

pub fn compute_address_len() -> u64 {
	unsafe { env::compute_address_len() }
}

pub fn balance_read(address_len: u64, address_ptr: u64) -> u64 {
	unsafe { env::balance_read(address_len, address_ptr) }
}

pub fn balance_transfer(recipient_address_len: u64, recipient_address_ptr: u64, value: u64) {
	unsafe { env::balance_transfer(recipient_address_len, recipient_address_ptr, value) }
}

pub fn pay() {
	unsafe { env::pay() }
}
