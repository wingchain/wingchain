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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use serde::Serialize;

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::DsaImpl;
use crypto::hash::{Hash as HashT, HashImpl};
use node_vm::errors::{BusinessError, VMError, VMResult};
use node_vm::{VMCallEnv, VMConfig, VMContext, VMContextEnv, VM};
use primitives::{Address, Balance, BlockNumber, DBValue, Hash};
use utils_test::test_accounts;

#[test]
fn test_vm_hello() {
	let context = Rc::new(get_context());
	let input = serde_json::to_vec(&"world".to_string()).unwrap();
	let result = vm_execute(context, "hello", input).unwrap();

	let result: String = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, "hello world".to_string());
}

#[test]
fn test_vm_error() {
	let context = Rc::new(get_context());
	let result = vm_execute(context, "error", vec![]);

	let error = result.unwrap_err();

	let expected_error: VMError = BusinessError::User {
		msg: "custom error".to_string(),
	}
	.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));
}

#[test]
fn test_vm_block_number() {
	let context = Rc::new(get_context());
	let result = vm_execute(context, "block_number", vec![]).unwrap();

	let result: u64 = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 10);
}

#[test]
fn test_vm_block_timestamp() {
	let context = Rc::new(get_context());
	let result = vm_execute(context, "block_timestamp", vec![]).unwrap();

	let result: u64 = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 12345);
}

#[test]
fn test_vm_tx_hash() {
	let context = Rc::new(get_context());
	let result = vm_execute(context, "tx_hash", vec![]).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	let hash = Arc::new(HashImpl::Blake2b256);
	let expected_tx_hash = {
		let mut out = vec![0u8; hash.length().into()];
		hash.hash(&mut out, &vec![1]);
		Hash(out)
	};

	assert_eq!(result, expected_tx_hash.0);
}

#[test]
fn test_vm_contract_address() {
	let context = Rc::new(get_context());
	let result = vm_execute(context.clone(), "contract_address", vec![]).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	let expected_address = context.contract_address.clone();

	assert_eq!(result, expected_address.0);
}

#[test]
fn test_vm_sender_address() {
	let context = Rc::new(get_context());
	let result = vm_execute(context.clone(), "sender_address", vec![]).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	let expected_address = context.sender_address.clone();

	assert_eq!(result, expected_address.0);
}

#[test]
fn test_vm_storage_get() {
	let context = Rc::new(get_context());
	let input = vec![1u8];
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context.clone(), "storage_get", input).unwrap();

	let result: Option<Vec<u8>> = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, Some(vec![2]));

	let input = vec![2u8];
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context, "storage_get", input).unwrap();

	let result: Option<Vec<u8>> = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, None);
}

#[test]
fn test_vm_storage_set() {
	#[derive(Serialize)]
	struct Input {
		key: Vec<u8>,
		value: Option<Vec<u8>>,
	}

	let context = Rc::new(get_context());
	let input = Input {
		key: vec![1u8],
		value: Some(vec![3u8]),
	};
	let input = serde_json::to_vec(&input).unwrap();
	let _result = vm_execute(context.clone(), "storage_set", input).unwrap();

	assert_eq!(context.payload.borrow().get(&vec![1u8]), Some(&vec![3]));

	let input = Input {
		key: vec![1u8],
		value: None,
	};
	let input = serde_json::to_vec(&input).unwrap();
	let _result = vm_execute(context.clone(), "storage_set", input).unwrap();

	assert_eq!(context.payload.borrow().get(&vec![1u8]), None);
}

#[test]
fn test_vm_event() {
	let context = Rc::new(get_context());
	let _result = vm_execute(context.clone(), "event", vec![]).unwrap();

	let event = context.events.borrow().get(0).unwrap().clone();
	let event = String::from_utf8(event).unwrap();

	assert_eq!(event, "{\"name\":\"MyEvent\"}");
}

#[test]
fn test_vm_compute_hash() {
	let context = Rc::new(get_context());
	let input = vec![1u8];
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context.clone(), "compute_hash", input).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	assert_eq!(
		result,
		vec![
			238u8, 21, 90, 206, 156, 64, 41, 32, 116, 203, 106, 255, 140, 156, 205, 210, 115, 200,
			22, 72, 255, 17, 73, 239, 54, 188, 234, 110, 187, 138, 62, 37
		]
	);

	let input = vec![2u8];
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context, "compute_hash", input).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	assert_eq!(
		result,
		vec![
			187u8, 48, 164, 44, 30, 98, 240, 175, 218, 95, 10, 78, 138, 86, 47, 122, 19, 162, 76,
			234, 0, 238, 129, 145, 123, 134, 184, 158, 128, 19, 20, 170
		]
	);
}

#[test]
fn test_vm_compute_address() {
	let context = Rc::new(get_context());
	let input = vec![1u8];
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context.clone(), "compute_address", input).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	assert_eq!(
		result,
		vec![
			202, 93, 63, 160, 166, 136, 114, 133, 239, 106, 168, 92, 177, 41, 96, 162, 182, 112,
			110, 0
		]
	);

	let input = vec![2u8];
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context, "compute_address", input).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	assert_eq!(
		result,
		vec![
			85, 237, 90, 196, 157, 121, 112, 231, 82, 44, 235, 194, 40, 99, 215, 178, 45, 122, 241,
			128
		]
	);
}

#[test]
fn test_vm_balance() {
	let context = Rc::new(get_context());

	let input = context.sender_address.clone().0;
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context.clone(), "balance", input).unwrap();

	let result: Balance = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 1000);

	let input = context.contract_address.clone().0;
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context.clone(), "balance", input).unwrap();

	let result: Balance = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 0);
}

fn get_context() -> TestVMContext {
	let hash = Arc::new(HashImpl::Blake2b256);
	let address = Arc::new(AddressImpl::Blake2b160);
	let dsa = Arc::new(DsaImpl::Ed25519);
	let payload = vec![(vec![1u8], vec![2u8])]
		.into_iter()
		.collect::<HashMap<_, _>>();
	let tx_hash = {
		let mut out = vec![0u8; hash.length().into()];
		hash.hash(&mut out, &vec![1]);
		Hash(out)
	};
	let contract_address = {
		let mut out = vec![0u8; address.length().into()];
		address.address(&mut out, &vec![1]);
		Address(out)
	};
	let (account1, _account2) = test_accounts(dsa, address.clone());
	let sender_address = account1.3;
	let balances = vec![(sender_address.clone(), 1000)]
		.into_iter()
		.collect::<HashMap<_, _>>();
	TestVMContext {
		block_number: 10,
		block_timestamp: 12345,
		tx_hash,
		contract_address,
		sender_address,
		payload: RefCell::new(payload),
		events: RefCell::new(Vec::new()),
		hash: hash.clone(),
		address: address.clone(),
		balances: RefCell::new(balances),
	}
}

fn vm_execute(context: Rc<dyn VMContext>, method: &str, input: Vec<u8>) -> VMResult<Vec<u8>> {
	let hash = Arc::new(HashImpl::Blake2b256);

	let config = VMConfig::default();

	let vm = VM::new(config, context).unwrap();

	let code =
		include_bytes!("../contract-samples/hello-world/pkg/contract_samples_hello_world_bg.wasm")
			.to_vec();
	let code_hash = {
		let mut out = vec![0; hash.length().into()];
		hash.hash(&mut out, &code);
		Hash(out)
	};

	let method = method.as_bytes().to_vec();
	let result = vm.execute(&code_hash, &code, method, input);
	result
}

#[derive(Clone)]
struct TestVMContext {
	block_number: BlockNumber,
	block_timestamp: u64,
	tx_hash: Hash,
	contract_address: Address,
	sender_address: Address,
	payload: RefCell<HashMap<Vec<u8>, DBValue>>,
	events: RefCell<Vec<Vec<u8>>>,
	hash: Arc<HashImpl>,
	address: Arc<AddressImpl>,
	balances: RefCell<HashMap<Address, Balance>>,
}

impl VMContext for TestVMContext {
	fn env(&self) -> Rc<VMContextEnv> {
		Rc::new(VMContextEnv {
			number: self.block_number,
			timestamp: self.block_timestamp,
		})
	}
	fn call_env(&self) -> Rc<VMCallEnv> {
		Rc::new(VMCallEnv {
			tx_hash: self.tx_hash.clone(),
			contract_address: self.contract_address.clone(),
			sender_address: self.sender_address.clone(),
		})
	}
	fn storage_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
		Ok(self.payload.borrow().get(key).cloned())
	}
	fn storage_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		match value {
			Some(value) => self.payload.borrow_mut().insert(key.to_vec(), value),
			None => self.payload.borrow_mut().remove(key),
		};
		Ok(())
	}
	fn emit_event(&self, event: Vec<u8>) -> VMResult<()> {
		self.events.borrow_mut().push(event);
		Ok(())
	}
	fn hash(&self, data: &[u8]) -> VMResult<Hash> {
		let mut out = vec![0u8; self.hash.length().into()];
		self.hash.hash(&mut out, data);
		Ok(Hash(out))
	}
	fn hash_len(&self) -> VMResult<u32> {
		let len: usize = self.hash.length().into();
		Ok(len as u32)
	}
	fn address(&self, data: &[u8]) -> VMResult<Address> {
		let mut out = vec![0u8; self.address.length().into()];
		self.address.address(&mut out, data);
		Ok(Address(out))
	}
	fn address_len(&self) -> VMResult<u32> {
		let len: usize = self.address.length().into();
		Ok(len as u32)
	}
	fn get_balance(&self, address: &Address) -> VMResult<Balance> {
		let balance = self.balances.borrow().get(address).unwrap_or(&0).clone();
		Ok(balance)
	}
}
