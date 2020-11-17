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
use crypto::hash::{Hash as HashT, HashImpl};
use node_vm::errors::{BusinessError, VMError, VMResult};
use node_vm::{VMCallEnv, VMConfig, VMContext, VMContextEnv, VM};
use primitives::{Address, BlockNumber, DBValue, Hash};

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

	assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_vm_storage_get() {
	let context = Rc::new(get_context());
	let input = vec![1u8];
	let result = vm_execute(context.clone(), "storage_get", input).unwrap();

	let result: Option<Vec<u8>> = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, Some(vec![2]));

	let input = vec![2u8];
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

fn get_context() -> TestVMContext {
	let hash = Arc::new(HashImpl::Blake2b256);
	let address = Arc::new(AddressImpl::Blake2b160);
	let payload = vec![(vec![1u8], vec![2u8])]
		.into_iter()
		.collect::<HashMap<_, _>>();
	TestVMContext {
		block_number: 10,
		block_timestamp: 12345,
		tx_hash: Hash(vec![1, 2, 3, 4, 5, 6, 7, 8]),
		payload: RefCell::new(payload),
		events: RefCell::new(Vec::new()),
		hash: hash.clone(),
		address: address.clone(),
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
	payload: RefCell<HashMap<Vec<u8>, DBValue>>,
	events: RefCell<Vec<Vec<u8>>>,
	hash: Arc<HashImpl>,
	address: Arc<AddressImpl>,
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
	fn address(&self, data: &[u8]) -> VMResult<Address> {
		let mut out = vec![0u8; self.address.length().into()];
		self.address.address(&mut out, data);
		Ok(Address(out))
	}
}
