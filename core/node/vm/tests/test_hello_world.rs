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

use serde::Serialize;

use node_vm::errors::{ContractError, VMError, VMResult};
use node_vm::{Mode, VMContext};

use crate::base::{storage_mint, TestExecutorContext, TestVMContext};

mod base;

#[test]
fn test_vm_hw_init() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3.clone(), 0, executor_context));

	let input = r#"{"value":"abc"}"#;
	let result = vm_execute(context.clone(), Mode::Init, "init", input).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	let input = r#""#;
	let result = vm_execute(context.clone(), Mode::Call, "get_value", input).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_hw_hello() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#"{"name": "world"}"#;
	let result = vm_execute(context, Mode::Call, "hello", input).unwrap();

	assert_eq!(result, r#""hello world""#.to_string());
}

#[test]
fn test_vm_hw_error() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#""#;
	let error = vm_execute(context, Mode::Call, "error", input).unwrap_err();

	let expected_error: VMError = ContractError::User {
		msg: "custom error".to_string(),
	}
	.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));
}

#[test]
fn test_vm_hw_get_env() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#""#;
	let result = vm_execute(context, Mode::Call, "get_env", input).unwrap();

	assert_eq!(result, r#"{"number":10,"timestamp":12345}"#.to_string());
}

#[test]
fn test_vm_hw_get_call_env() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#""#;
	let result = vm_execute(context, Mode::Call, "get_call_env", input).unwrap();

	assert_eq!(
		result,
		r#"{"tx_hash":"ee155ace9c40292074cb6aff8c9ccdd273c81648ff1149ef36bcea6ebb8a3e25"}"#
			.to_string()
	);
}

#[test]
fn test_vm_hw_get_contract_env() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	storage_mint(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = Rc::new(TestVMContext::new(account1.3, 10, executor_context));

	let input = r#""#;
	let result = vm_execute(context, Mode::Call, "get_contract_env", input).unwrap();

	assert_eq!(result, r#"{"contract_address":"ca5d3fa0a6887285ef6aa85cb12960a2b6706e00","sender_address":"b4decd5a5f8f2ba708f8ced72eec89f44f3be96a","pay_value":10}"#.to_string());
}

#[test]
fn test_vm_hw_value() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#""#;
	let result = vm_execute(context.clone(), Mode::Call, "get_value", input).unwrap();
	assert_eq!(result, r#"{"value":null}"#.to_string());

	let input = r#"{"value":"abc"}"#;
	let result = vm_execute(context.clone(), Mode::Call, "set_value", input).unwrap();
	assert_eq!(result, r#"null"#.to_string());

	let input = r#""#;
	let result = vm_execute(context.clone(), Mode::Call, "get_value", input).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_hw_map() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#"{"key":[1,2,3]}"#;
	let result = vm_execute(context.clone(), Mode::Call, "get_map", input).unwrap();
	assert_eq!(result, r#"{"value":null}"#.to_string());

	let input = r#"{"key":[1,2,3],"value":"abc"}"#;
	let result = vm_execute(context.clone(), Mode::Call, "set_map", input).unwrap();
	assert_eq!(result, r#"null"#.to_string());

	let input = r#"{"key":[1,2,3]}"#;
	let result = vm_execute(context.clone(), Mode::Call, "get_map", input).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_hw_event() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#""#;
	let _result = vm_execute(context.clone(), Mode::Call, "event", input).unwrap();

	let events = context.drain_events().unwrap();
	let event = events.get(0).unwrap().clone();
	let event = String::from_utf8(event).unwrap();

	assert_eq!(event, "{\"name\":\"MyEvent\",\"data\":{\"foo\":\"bar\"}}");
}

#[test]
fn test_vm_hw_hash() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#"{"data":[1]}"#;
	let result = vm_execute(context, Mode::Call, "hash", input).unwrap();

	assert_eq!(
		result,
		r#""ee155ace9c40292074cb6aff8c9ccdd273c81648ff1149ef36bcea6ebb8a3e25""#.to_string()
	);
}

#[test]
fn test_vm_hw_address() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#"{"data":[1]}"#;
	let result = vm_execute(context, Mode::Call, "address", input).unwrap();

	assert_eq!(
		result,
		r#""ca5d3fa0a6887285ef6aa85cb12960a2b6706e00""#.to_string()
	);
}

#[test]
fn test_vm_hw_validate_address() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#"{"address":"aa"}"#;
	let error = vm_execute(context.clone(), Mode::Call, "validate_address", input).unwrap_err();

	let expected_error: VMError = ContractError::InvalidAddress.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));

	let input = r#"{"address":"aa"}"#;
	let result = vm_execute(context, Mode::Call, "validate_address_ea", input).unwrap();

	assert_eq!(
		result,
		r#""false: ContractError: InvalidAddress""#.to_string()
	);
}

#[test]
fn test_vm_hw_balance() {
	let (account1, _account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	storage_mint(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = Rc::new(TestVMContext::new(account1.3, 0, executor_context));

	let input = r#"{"address":"b4decd5a5f8f2ba708f8ced72eec89f44f3be96a"}"#;
	let result = vm_execute(context, Mode::Call, "get_balance", input).unwrap();

	assert_eq!(result, r#"1000"#.to_string());
}

#[test]
fn test_vm_hw_balance_transfer() {
	#[derive(Serialize)]
	struct Input {
		recipient: Vec<u8>,
		value: u64,
	}

	let (account1, account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	storage_mint(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = Rc::new(TestVMContext::new(
		account1.3.clone(),
		100,
		executor_context.clone(),
	));

	let input = format!(
		r#"{{"recipient":"{}", "value": 10}}"#,
		hex::encode((account2.3).0.clone())
	);
	let result = vm_execute(context.clone(), Mode::Call, "balance_transfer", &input);

	// apply
	match result {
		Ok(_) => {
			context
				.payload_apply(context.payload_drain_buffer().unwrap())
				.unwrap();
		}
		Err(_) => {
			context.payload_drain_buffer().unwrap();
		}
	}

	// shift to new context
	let context = Rc::new(TestVMContext::new(account1.3.clone(), 0, executor_context));

	let account1_balance = context.module_balance_get(&account1.3).unwrap();
	let contract_balance = context
		.module_balance_get(&context.contract_env().contract_address)
		.unwrap();
	let account2_balance = context.module_balance_get(&account2.3).unwrap();

	assert_eq!(account1_balance, 900);
	assert_eq!(contract_balance, 90);
	assert_eq!(account2_balance, 10);
}

#[test]
fn test_vm_hw_balance_transfer_failed() {
	#[derive(Serialize)]
	struct Input {
		recipient: Vec<u8>,
		value: u64,
	}

	let (account1, account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	storage_mint(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = Rc::new(TestVMContext::new(
		account1.3.clone(),
		100,
		executor_context.clone(),
	));

	let input = format!(
		r#"{{"recipient":"{}", "value": 200}}"#,
		hex::encode((account2.3).0.clone())
	);
	let result = vm_execute(context.clone(), Mode::Call, "balance_transfer", &input);

	// apply
	match result {
		Ok(_) => {
			context
				.payload_apply(context.payload_drain_buffer().unwrap())
				.unwrap();
		}
		Err(_) => {
			context.payload_drain_buffer().unwrap();
		}
	}

	// shift to new context
	let context = Rc::new(TestVMContext::new(account1.3.clone(), 0, executor_context));

	let account1_balance = context.module_balance_get(&account1.3).unwrap();
	let contract_balance = context
		.module_balance_get(&context.contract_env().contract_address)
		.unwrap();
	let account2_balance = context.module_balance_get(&account2.3).unwrap();

	assert_eq!(account1_balance, 1000);
	assert_eq!(contract_balance, 0);
	assert_eq!(account2_balance, 0);
}

#[test]
fn test_vm_hw_balance_transfer_partial_failed() {
	#[derive(Serialize)]
	struct Input {
		recipient: Vec<u8>,
		value: u64,
	}

	let (account1, account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();
	storage_mint(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = Rc::new(TestVMContext::new(
		account1.3.clone(),
		100,
		executor_context.clone(),
	));

	let input = format!(
		r#"{{"recipient":"{}", "value": 200}}"#,
		hex::encode((account2.3).0.clone())
	);
	let result = vm_execute(context.clone(), Mode::Call, "balance_transfer_ea", &input);

	println!("result: {:?}", result);

	// apply
	match result {
		Ok(_) => {
			context
				.payload_apply(context.payload_drain_buffer().unwrap())
				.unwrap();
		}
		Err(_) => {
			context.payload_drain_buffer().unwrap();
		}
	}

	// shift to new context
	let context = Rc::new(TestVMContext::new(account1.3.clone(), 0, executor_context));

	let account1_balance = context.module_balance_get(&account1.3).unwrap();
	let contract_balance = context
		.module_balance_get(&context.contract_env().contract_address)
		.unwrap();
	let account2_balance = context.module_balance_get(&account2.3).unwrap();

	assert_eq!(account1_balance, 900);
	assert_eq!(contract_balance, 100);
	assert_eq!(account2_balance, 0);
}

fn vm_execute(
	context: Rc<dyn VMContext>,
	mode: Mode,
	method: &str,
	input: &str,
) -> VMResult<String> {
	let code =
		include_bytes!("../contract-samples/hello-world/pkg/contract_samples_hello_world_bg.wasm");
	let code = &code[..];
	base::vm_execute(code, context, mode, method, input)
}
