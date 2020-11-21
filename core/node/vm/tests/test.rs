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
use node_vm::errors::{ContractError, VMError, VMResult};
use node_vm::{VMCallEnv, VMConfig, VMContext, VMContextEnv, VMContractEnv, VM};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Balance, DBKey, DBValue, Hash};
use utils_test::test_accounts;

#[test]
fn test_vm_hello() {
	let context = Rc::new(init_context(0));
	let input = r#"{"name": "world"}"#;
	let result = vm_execute(context, "hello", input).unwrap();

	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(result, r#""hello world""#.to_string());
}

#[test]
fn test_vm_error() {
	let context = Rc::new(init_context(0));

	let input = r#""#;
	let error = vm_execute(context, "error", input).unwrap_err();

	let expected_error: VMError = ContractError::User {
		msg: "custom error".to_string(),
	}
	.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));
}

#[test]
fn test_vm_get_env() {
	let context = Rc::new(init_context(0));
	let input = r#""#;
	let result = vm_execute(context, "get_env", input).unwrap();

	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(result, r#"{"number":10,"timestamp":12345}"#.to_string());
}

#[test]
fn test_vm_get_call_env() {
	let context = Rc::new(init_context(0));
	let input = r#""#;
	let result = vm_execute(context, "get_call_env", input).unwrap();

	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(
		result,
		r#"{"tx_hash":"ee155ace9c40292074cb6aff8c9ccdd273c81648ff1149ef36bcea6ebb8a3e25"}"#
			.to_string()
	);
}

#[test]
fn test_vm_get_contract_env() {
	let context = Rc::new(init_context(10));
	let input = r#""#;
	let result = vm_execute(context, "get_contract_env", input).unwrap();

	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(result, r#"{"contract_address":"ca5d3fa0a6887285ef6aa85cb12960a2b6706e00","sender_address":"b4decd5a5f8f2ba708f8ced72eec89f44f3be96a","pay_value":10}"#.to_string());
}

#[test]
fn test_vm_value() {
	let context = Rc::new(init_context(0));

	let input = r#""#;
	let result = vm_execute(context.clone(), "get_value", input).unwrap();
	let result: String = String::from_utf8(result).unwrap();
	assert_eq!(result, r#"{"value":null}"#.to_string());

	let input = r#"{"value":"abc"}"#;
	let result = vm_execute(context.clone(), "set_value", input).unwrap();
	let result: String = String::from_utf8(result).unwrap();
	assert_eq!(result, r#"null"#.to_string());

	let input = r#""#;
	let result = vm_execute(context.clone(), "get_value", input).unwrap();
	let result: String = String::from_utf8(result).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_map() {
	let context = Rc::new(init_context(0));

	let input = r#"{"key":[1,2,3]}"#;
	let result = vm_execute(context.clone(), "get_map", input).unwrap();
	let result: String = String::from_utf8(result).unwrap();
	assert_eq!(result, r#"{"value":null}"#.to_string());

	let input = r#"{"key":[1,2,3],"value":"abc"}"#;
	let result = vm_execute(context.clone(), "set_map", input).unwrap();
	let result: String = String::from_utf8(result).unwrap();
	assert_eq!(result, r#"null"#.to_string());

	let input = r#"{"key":[1,2,3]}"#;
	let result = vm_execute(context.clone(), "get_map", input).unwrap();
	let result: String = String::from_utf8(result).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_event() {
	let context = Rc::new(init_context(0));

	let input = r#""#;
	let _result = vm_execute(context.clone(), "event", input).unwrap();

	let event = context.events.borrow().get(0).unwrap().clone();
	let event = String::from_utf8(event).unwrap();

	assert_eq!(event, "{\"name\":\"MyEvent\"}");
}

#[test]
fn test_vm_hash() {
	let context = Rc::new(init_context(0));
	let input = r#"{"data":[1]}"#;
	let result = vm_execute(context, "hash", input).unwrap();

	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(
		result,
		r#""ee155ace9c40292074cb6aff8c9ccdd273c81648ff1149ef36bcea6ebb8a3e25""#.to_string()
	);
}

#[test]
fn test_vm_address() {
	let context = Rc::new(init_context(0));
	let input = r#"{"data":[1]}"#;
	let result = vm_execute(context, "address", input).unwrap();

	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(
		result,
		r#""ca5d3fa0a6887285ef6aa85cb12960a2b6706e00""#.to_string()
	);
}

#[test]
fn test_vm_validate_address() {
	let context = Rc::new(init_context(0));
	let input = r#"{"address":"aa"}"#;
	let error = vm_execute(context.clone(), "validate_address", input).unwrap_err();

	let expected_error: VMError = ContractError::InvalidAddress.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));

	let input = r#"{"address":"aa"}"#;
	let result = vm_execute(context, "validate_address_ea", input).unwrap();
	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(
		result,
		r#""false: ContractError: InvalidAddress""#.to_string()
	);
}

#[test]
fn test_vm_balance() {
	let context = Rc::new(init_context(0));
	let input = r#"{"address":"b4decd5a5f8f2ba708f8ced72eec89f44f3be96a"}"#;
	let result = vm_execute(context, "get_balance", input).unwrap();

	let result: String = String::from_utf8(result).unwrap();

	assert_eq!(result, r#"1000"#.to_string());
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

	let input = format!(
		r#"{{"recipient":"{}", "value": 10}}"#,
		hex::encode((account2.3).0.clone())
	);
	let _result = vm_execute(context.clone(), "balance_transfer", &input).unwrap();

	for (k, v) in context.buffer.borrow_mut().drain() {
		if let Some(v) = v {
			context.payload.borrow_mut().insert(k.to_vec(), v);
		}
	}

	let account1_balance = context.balance_get(&account1.3).unwrap();
	let contract_balance = context
		.balance_get(&context.contract_env().contract_address)
		.unwrap();
	let account2_balance = context.balance_get(&account2.3).unwrap();

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

fn vm_execute(context: Rc<dyn VMContext>, method: &str, input: &str) -> VMResult<Vec<u8>> {
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
	let input = input.as_bytes().to_vec();
	let result = vm.execute(&code_hash, &code, method, input)?;
	Ok(result)
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
	fn address(&self, data: &[u8]) -> VMResult<Address> {
		let mut out = vec![0u8; self.address.length().into()];
		self.address.address(&mut out, data);
		Ok(Address(out))
	}
	fn validate_address(&self, address: &Address) -> VMResult<()> {
		let address_len: usize = self.address.length().into();
		if address.0.len() != address_len {
			return Err(ContractError::InvalidAddress.into());
		}
		Ok(())
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
			return Err(ContractError::Transfer.into());
		}

		sender_balance -= value;
		recipient_balance += value;

		self.payload_storage_map_set(b"balance", b"balance", sender, &sender_balance)?;
		self.payload_storage_map_set(b"balance", b"balance", recipient, &recipient_balance)?;
		Ok(())
	}
}
