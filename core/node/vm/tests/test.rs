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

use std::rc::Rc;
use std::sync::Arc;

use crypto::hash::{Hash as HashT, HashImpl};
use node_vm::errors::{BusinessError, VMError, VMResult};
use node_vm::{VMCallEnv, VMConfig, VMContext, VMContextEnv, VM};
use primitives::{BlockNumber, Hash};

#[test]
fn test_vm_hello() {
	let input = serde_json::to_vec(&"world".to_string()).unwrap();
	let result = test_vm_execute("hello", input).unwrap();

	let result: String = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, "hello world".to_string());
}

#[test]
fn test_vm_error() {
	let result = test_vm_execute("error", vec![]);

	let error = result.unwrap_err();

	let expected_error: VMError = BusinessError::User {
		msg: "custom error".to_string(),
	}
	.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));
}

#[test]
fn test_vm_block_number() {
	let result = test_vm_execute("block_number", vec![]).unwrap();

	let result: u64 = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 10);
}

#[test]
fn test_vm_block_timestamp() {
	let result = test_vm_execute("block_timestamp", vec![]).unwrap();

	let result: u64 = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 12345);
}

#[test]
fn test_vm_tx_hash() {
	let result = test_vm_execute("tx_hash", vec![]).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

fn test_vm_execute(method: &str, input: Vec<u8>) -> VMResult<Vec<u8>> {
	let hash = Arc::new(HashImpl::Blake2b256);

	let config = VMConfig::default();

	let context = TestVMContext {
		block_number: 10,
		block_timestamp: 12345,
		tx_hash: Hash(vec![1, 2, 3, 4, 5, 6, 7, 8]),
	};

	let vm = VM::new(config, Rc::new(context)).unwrap();

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
}
