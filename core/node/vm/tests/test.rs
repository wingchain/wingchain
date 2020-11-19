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
use node_vm::{VMCallEnv, VMConfig, VMContext, VMContextEnv, VMContractEnv, VM};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Balance, DBKey, DBValue, Hash};
use utils_test::test_accounts;

#[test]
fn test_vm_hello() {
	let context = Rc::new(init_context(0));
	let input = serde_json::to_vec(&"world".to_string()).unwrap();
	let result = vm_execute(context, "hello", input).unwrap();

	let result: String = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, "hello world".to_string());
}

#[test]
fn test_vm_error() {
	let context = Rc::new(init_context(0));
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
	let context = Rc::new(init_context(0));
	let result = vm_execute(context.clone(), "block_number", vec![]).unwrap();

	let result: u64 = serde_json::from_slice(&result).unwrap();

	let expected_result = context.env().number;

	assert_eq!(result, expected_result);
}

#[test]
fn test_vm_block_timestamp() {
	let context = Rc::new(init_context(0));
	let result = vm_execute(context.clone(), "block_timestamp", vec![]).unwrap();

	let result: u64 = serde_json::from_slice(&result).unwrap();

	let expected_result = context.env().timestamp;

	assert_eq!(result, expected_result);
}

#[test]
fn test_vm_tx_hash() {
	let context = Rc::new(init_context(0));
	let result = vm_execute(context.clone(), "tx_hash", vec![]).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	let expected_tx_hash = context.call_env().tx_hash.clone();

	assert_eq!(result, expected_tx_hash.0);
}

#[test]
fn test_vm_contract_address() {
	let context = Rc::new(init_context(0));
	let result = vm_execute(context.clone(), "contract_address", vec![]).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	let expected_address = context.contract_env().contract_address.clone();

	assert_eq!(result, expected_address.0);
}

#[test]
fn test_vm_sender_address() {
	let context = Rc::new(init_context(0));
	let result = vm_execute(context.clone(), "sender_address", vec![]).unwrap();

	let result: Vec<u8> = serde_json::from_slice(&result).unwrap();

	let expected_address = context.contract_env().sender_address.clone();

	assert_eq!(result, expected_address.0);
}

#[test]
fn test_vm_pay_value() {
	let context = Rc::new(init_context(10));
	let result = vm_execute(context.clone(), "pay_value", vec![]).unwrap();

	let result: Balance = serde_json::from_slice(&result).unwrap();

	let expected_pay_value = context.contract_env().pay_value.clone();

	assert_eq!(result, expected_pay_value);
}

#[test]
fn test_vm_storage_get() {
	let context = Rc::new(init_context(0));
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

	let context = Rc::new(init_context(0));
	let input = Input {
		key: vec![1u8],
		value: Some(vec![3u8]),
	};
	let input = serde_json::to_vec(&input).unwrap();
	let _result = vm_execute(context.clone(), "storage_set", input).unwrap();

	assert_eq!(
		context
			.buffer
			.borrow()
			.get(&DBKey::from_slice(&vec![1u8]))
			.unwrap(),
		&Some(vec![3])
	);

	let input = Input {
		key: vec![1u8],
		value: None,
	};
	let input = serde_json::to_vec(&input).unwrap();
	let _result = vm_execute(context.clone(), "storage_set", input).unwrap();

	assert_eq!(
		context
			.buffer
			.borrow()
			.get(&DBKey::from_slice(&vec![1u8]))
			.unwrap(),
		&None
	);
}

#[test]
fn test_vm_event() {
	let context = Rc::new(init_context(0));
	let _result = vm_execute(context.clone(), "event", vec![]).unwrap();

	let event = context.events.borrow().get(0).unwrap().clone();
	let event = String::from_utf8(event).unwrap();

	assert_eq!(event, "{\"name\":\"MyEvent\"}");
}

#[test]
fn test_vm_compute_hash() {
	let context = Rc::new(init_context(0));
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
	let context = Rc::new(init_context(0));
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
fn test_vm_balance_get() {
	let context = Rc::new(init_context(0));

	let input = context.contract_env().sender_address.clone().0;
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context.clone(), "balance", input).unwrap();

	let result: Balance = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 1000);

	let input = context.contract_env().contract_address.clone().0;
	let input = serde_json::to_vec(&input).unwrap();
	let result = vm_execute(context.clone(), "balance", input).unwrap();

	let result: Balance = serde_json::from_slice(&result).unwrap();

	assert_eq!(result, 0);
}

#[test]
fn test_vm_balance_transfer() {
	#[derive(Serialize)]
	struct Input {
		recipient: Vec<u8>,
		value: u64,
	}

	let context = Rc::new(init_context(100));
	let address = Arc::new(AddressImpl::Blake2b160);
	let dsa = Arc::new(DsaImpl::Ed25519);
	let (account1, account2) = test_accounts(dsa, address);

	let input = Input {
		recipient: account2.3.clone().0,
		value: 10,
	};
	let input = serde_json::to_vec(&input).unwrap();
	let _result = vm_execute(context.clone(), "balance_transfer", input).unwrap();

	for (k, v) in context.buffer.borrow_mut().drain() {
		if let Some(v) = v {
			context.payload.borrow_mut().insert(k.to_vec(), v);
		}
	}

	let account1_balance: Balance = context
		.payload_storage_map_get(b"balance", b"balance", &account1.3)
		.unwrap()
		.unwrap();
	let contract_balance: Balance = context
		.payload_storage_map_get(
			b"balance",
			b"balance",
			&context.contract_env().contract_address,
		)
		.unwrap()
		.unwrap();
	let account2_balance: Balance = context
		.payload_storage_map_get(b"balance", b"balance", &account2.3)
		.unwrap()
		.unwrap();

	assert_eq!(account1_balance, 900);
	assert_eq!(contract_balance, 90);
	assert_eq!(account2_balance, 10);
}

fn init_context(pay_value: Balance) -> TestVMContext {
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
	let context = TestVMContext {
		env: Rc::new(VMContextEnv {
			number: 10,
			timestamp: 12345,
		}),
		call_env: Rc::new(VMCallEnv { tx_hash }),
		contract_env: Rc::new(VMContractEnv {
			contract_address,
			sender_address: sender_address.clone(),
			pay_value,
		}),
		payload: RefCell::new(payload),
		buffer: RefCell::new(HashMap::new()),
		events: RefCell::new(Vec::new()),
		hash: hash.clone(),
		address: address.clone(),
	};

	let balance: Balance = 1000;
	context
		.payload_storage_map_set(b"balance", b"balance", &sender_address, &balance)
		.unwrap();
	for (k, v) in context.buffer.borrow_mut().drain() {
		if let Some(v) = v {
			context.payload.borrow_mut().insert(k.to_vec(), v);
		}
	}

	context
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
	env: Rc<VMContextEnv>,
	call_env: Rc<VMCallEnv>,
	contract_env: Rc<VMContractEnv>,
	payload: RefCell<HashMap<Vec<u8>, DBValue>>,
	buffer: RefCell<HashMap<DBKey, Option<DBValue>>>,
	events: RefCell<Vec<Vec<u8>>>,
	hash: Arc<HashImpl>,
	address: Arc<AddressImpl>,
}

const SEPARATOR: &[u8] = b"_";

impl TestVMContext {
	fn payload_storage_map_get<K: Encode, V: Decode>(
		&self,
		module_name: &[u8],
		storage_name: &[u8],
		key: &K,
	) -> VMResult<Option<V>> {
		let key = codec::encode(key)?;
		let key = &[module_name, SEPARATOR, storage_name, SEPARATOR, &key].concat();
		let value = self.payload_get(key)?;
		let value = match value {
			Some(value) => {
				let value = codec::decode(&value[..])?;
				Ok(Some(value))
			}
			None => Ok(None),
		};

		value
	}
	fn payload_storage_map_set<K: Encode, V: Encode>(
		&self,
		module_name: &[u8],
		storage_name: &[u8],
		key: &K,
		value: &V,
	) -> VMResult<()> {
		let key = codec::encode(key)?;
		let key = &[module_name, SEPARATOR, storage_name, SEPARATOR, &key].concat();

		let value = codec::encode(value)?;
		self.payload_set(key, Some(value))?;
		Ok(())
	}
}

impl VMContext for TestVMContext {
	fn env(&self) -> Rc<VMContextEnv> {
		self.env.clone()
	}
	fn call_env(&self) -> Rc<VMCallEnv> {
		self.call_env.clone()
	}
	fn contract_env(&self) -> Rc<VMContractEnv> {
		self.contract_env.clone()
	}
	fn payload_get(&self, key: &[u8]) -> VMResult<Option<DBValue>> {
		if let Some(value) = self.buffer.borrow().get(&DBKey::from_slice(key)) {
			return Ok(value.clone());
		}
		Ok(self.payload.borrow().get(key).cloned())
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> VMResult<()> {
		let mut buffer = self.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_drain_buffer(&self) -> VMResult<Vec<(DBKey, Option<DBValue>)>> {
		let buffer = self.buffer.borrow_mut().drain().collect();
		Ok(buffer)
	}
	fn emit_event(&self, event: Vec<u8>) -> VMResult<()> {
		self.events.borrow_mut().push(event);
		Ok(())
	}
	fn drain_events(&self) -> VMResult<Vec<Vec<u8>>> {
		let events = self.events.borrow_mut().drain(..).collect();
		Ok(events)
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
	fn balance_get(&self, address: &Address) -> VMResult<Balance> {
		let balance: Option<Balance> =
			self.payload_storage_map_get(b"balance", b"balance", address)?;
		let balance = balance.unwrap_or(0);
		Ok(balance)
	}
	fn balance_transfer(
		&self,
		sender: &Address,
		recipient: &Address,
		value: Balance,
	) -> VMResult<()> {
		let mut sender_balance = self.balance_get(sender)?;
		let mut recipient_balance = self.balance_get(recipient)?;

		if sender_balance < value {
			return Err(BusinessError::Transfer.into());
		}

		sender_balance -= value;
		recipient_balance += value;

		self.payload_storage_map_set(b"balance", b"balance", sender, &sender_balance)?;
		self.payload_storage_map_set(b"balance", b"balance", recipient, &recipient_balance)?;
		Ok(())
	}
}
