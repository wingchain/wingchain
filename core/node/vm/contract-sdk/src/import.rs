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
		pub fn share_read(share_id: u64, ptr: u64);
		pub fn share_len(share_id: u64) -> u64;
		pub fn share_write(data_len: u64, data_ptr: u64, share_id: u64);
		pub fn method_read(ptr: u64);
		pub fn params_read(ptr: u64);
		pub fn pay_value_read() -> u64;
		pub fn result_write(len: u64, ptr: u64);
		pub fn error_return(len: u64, ptr: u64);
		pub fn env_block_number() -> u64;
		pub fn env_block_timestamp() -> u64;
		pub fn env_tx_hash_read(share_id: u64) -> u64;
		pub fn env_contract_address_read(share_id: u64) -> u64;
		pub fn env_sender_address_read(share_id: u64) -> u64;
		pub fn storage_read(key_len: u64, key_ptr: u64, share_id: u64) -> u64;
		pub fn storage_write(
			key_len: u64,
			key_ptr: u64,
			value_exist: u64,
			value_len: u64,
			value_ptr: u64,
		);
		pub fn event_write(len: u64, ptr: u64);
		pub fn util_hash(data_len: u64, data_ptr: u64, share_id: u64);
		pub fn util_address(data_len: u64, data_ptr: u64, share_id: u64);
		pub fn util_validate_address(data_len: u64, data_ptr: u64);
		pub fn util_validate_address_ea(data_len: u64, data_ptr: u64, error_share_id: u64) -> u64;
		pub fn balance_read(address_len: u64, address_ptr: u64) -> u64;
		pub fn balance_transfer(recipient_address_len: u64, recipient_address_ptr: u64, value: u64);
		pub fn balance_transfer_ea(
			recipient_address_len: u64,
			recipient_address_ptr: u64,
			value: u64,
			error_share_id: u64,
		) -> u64;
		pub fn pay();
		pub fn contract_execute(
			contract_address_len: u64,
			contract_address_ptr: u64,
			method_len: u64,
			method_ptr: u64,
			params_len: u64,
			params_ptr: u64,
			pay_value: u64,
			share_id: u64,
		);
		pub fn contract_execute_ea(
			contract_address_len: u64,
			contract_address_ptr: u64,
			method_len: u64,
			method_ptr: u64,
			params_len: u64,
			params_ptr: u64,
			pay_value: u64,
			share_id: u64,
			error_share_id: u64,
		) -> u64;
	}
}

pub fn share_read(share_id: u64, ptr: u64) {
	unsafe { env::share_read(share_id, ptr) }
}

pub fn share_len(share_id: u64) -> u64 {
	unsafe { env::share_len(share_id) }
}

pub fn share_write(data_len: u64, data_ptr: u64, share_id: u64) {
	unsafe { env::share_write(data_len, data_ptr, share_id) }
}

pub fn method_read(ptr: u64) {
	unsafe { env::method_read(ptr) }
}

pub fn params_read(ptr: u64) {
	unsafe { env::params_read(ptr) }
}

pub fn pay_value_read() -> u64 {
	unsafe { env::pay_value_read() }
}

pub fn result_write(len: u64, ptr: u64) {
	unsafe { env::result_write(len, ptr) }
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

pub fn env_tx_hash_read(share_id: u64) -> u64 {
	unsafe { env::env_tx_hash_read(share_id) }
}

pub fn env_contract_address_read(share_id: u64) -> u64 {
	unsafe { env::env_contract_address_read(share_id) }
}

pub fn env_sender_address_read(share_id: u64) -> u64 {
	unsafe { env::env_sender_address_read(share_id) }
}

pub fn storage_read(key_len: u64, key_ptr: u64, share_id: u64) -> u64 {
	unsafe { env::storage_read(key_len, key_ptr, share_id) }
}

pub fn storage_write(key_len: u64, key_ptr: u64, value_exist: u64, value_len: u64, value_ptr: u64) {
	unsafe { env::storage_write(key_len, key_ptr, value_exist, value_len, value_ptr) }
}

pub fn event_write(len: u64, ptr: u64) {
	unsafe { env::event_write(len, ptr) }
}

pub fn util_hash(data_len: u64, data_ptr: u64, share_id: u64) {
	unsafe { env::util_hash(data_len, data_ptr, share_id) }
}

pub fn util_address(data_len: u64, data_ptr: u64, share_id: u64) {
	unsafe { env::util_address(data_len, data_ptr, share_id) }
}

pub fn util_validate_address(data_len: u64, data_ptr: u64) {
	unsafe { env::util_validate_address(data_len, data_ptr) }
}

pub fn util_validate_address_ea(data_len: u64, data_ptr: u64, error_share_id: u64) -> u64 {
	unsafe { env::util_validate_address_ea(data_len, data_ptr, error_share_id) }
}

pub fn balance_read(address_len: u64, address_ptr: u64) -> u64 {
	unsafe { env::balance_read(address_len, address_ptr) }
}

pub fn balance_transfer(recipient_address_len: u64, recipient_address_ptr: u64, value: u64) {
	unsafe { env::balance_transfer(recipient_address_len, recipient_address_ptr, value) }
}

pub fn balance_transfer_ea(
	recipient_address_len: u64,
	recipient_address_ptr: u64,
	value: u64,
	error_share_id: u64,
) -> u64 {
	unsafe {
		env::balance_transfer_ea(
			recipient_address_len,
			recipient_address_ptr,
			value,
			error_share_id,
		)
	}
}

pub fn pay() {
	unsafe { env::pay() }
}

pub fn contract_execute(
	contract_address_len: u64,
	contract_address_ptr: u64,
	method_len: u64,
	method_ptr: u64,
	params_len: u64,
	params_ptr: u64,
	pay_value: u64,
	share_id: u64,
) {
	unsafe {
		env::contract_execute(
			contract_address_len,
			contract_address_ptr,
			method_len,
			method_ptr,
			params_len,
			params_ptr,
			pay_value,
			share_id,
		)
	}
}

pub fn contract_execute_ea(
	contract_address_len: u64,
	contract_address_ptr: u64,
	method_len: u64,
	method_ptr: u64,
	params_len: u64,
	params_ptr: u64,
	pay_value: u64,
	share_id: u64,
	error_share_id: u64,
) -> u64 {
	unsafe {
		env::contract_execute_ea(
			contract_address_len,
			contract_address_ptr,
			method_len,
			method_ptr,
			params_len,
			params_ptr,
			pay_value,
			share_id,
			error_share_id,
		)
	}
}
