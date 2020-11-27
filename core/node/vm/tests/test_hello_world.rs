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

use std::borrow::Cow;
use std::rc::Rc;

use node_vm::errors::{ContractError, PreCompileError, VMError, VMResult};
use node_vm::{Mode, VMContext};
use primitives::{Address, Balance, Hash};

use crate::base::{endow, ExecutorContext, TestExecutorContext, TestVMContext};

mod base;

#[test]
fn test_vm_hw_validate_contract() {
	// params deserialize error
	let params = r#"{"value1":"abc"}"#;
	let error = vm_validate(None, Mode::Init, "init", params, 0).unwrap_err();

	let expected_error: VMError = ContractError::InvalidParams.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));

	// not payable error
	let params = r#"{"value":"abc"}"#;
	let error = vm_validate(None, Mode::Init, "init", params, 10).unwrap_err();

	let expected_error: VMError = ContractError::NotPayable.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));

	// pre compile error
	let params = r#"{"value":"abc"}"#;
	let error = vm_validate(Some(vec![1; 1024]), Mode::Init, "init", params, 0).unwrap_err();

	let expected_error: VMError = PreCompileError::ValidationError {
		msg: "Bad magic number (at offset 0)".to_string(),
	}
	.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));
}

#[test]
fn test_vm_hw_init() {
	let (account1, _account2) = base::test_accounts();
	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3.clone()),
		executor_context,
	);

	let params = r#"{"value":"abc"}"#.as_bytes();
	let result = vm_execute(&context, Mode::Init, "init", params, 0).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	let params = r#""#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_value", params, 0).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_hw_hello() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#"{"name": "world"}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "hello", params, 0).unwrap();

	assert_eq!(result, r#""hello world""#.to_string());
}

#[test]
fn test_vm_hw_pay_value() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	endow(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#""#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "pay_value", params, 100).unwrap();

	assert_eq!(result, r#"100"#.to_string());
}

#[test]
fn test_vm_hw_error() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#""#.as_bytes();
	let error = vm_execute(&context, Mode::Call, "error", params, 0).unwrap_err();

	let expected_error: VMError = ContractError::User {
		msg: "Custom error".to_string(),
	}
	.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));
}

#[test]
fn test_vm_hw_get_env() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#""#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_env", params, 0).unwrap();

	assert_eq!(result, r#"{"number":10,"timestamp":12345}"#.to_string());
}

#[test]
fn test_vm_hw_get_call_env() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#""#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_call_env", params, 0).unwrap();

	assert_eq!(result, r#"{"tx_hash":"01"}"#.to_string());
}

#[test]
fn test_vm_hw_get_contract_env() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	endow(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#""#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_contract_env", params, 0).unwrap();

	assert_eq!(
		result,
		r#"{"contract_address":"01","sender_address":"b4decd5a5f8f2ba708f8ced72eec89f44f3be96a"}"#
			.to_string()
	);
}

#[test]
fn test_vm_hw_value() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#""#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_value", params, 0).unwrap();
	assert_eq!(result, r#"{"value":null}"#.to_string());

	let params = r#"{"value":"abc"}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "set_value", params, 0).unwrap();
	assert_eq!(result, r#"null"#.to_string());

	let params = r#""#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_value", params, 0).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_hw_map() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#"{"key":[1,2,3]}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_map", params, 0).unwrap();
	assert_eq!(result, r#"{"value":null}"#.to_string());

	let params = r#"{"key":[1,2,3],"value":"abc"}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "set_map", params, 0).unwrap();
	assert_eq!(result, r#"null"#.to_string());

	let params = r#"{"key":[1,2,3]}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_map", params, 0).unwrap();
	assert_eq!(result, r#"{"value":"abc"}"#.to_string());
}

#[test]
fn test_vm_hw_event() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context.clone(),
	);

	let params = r#""#.as_bytes();
	let _result = vm_execute(&context, Mode::Call, "event", params, 0).unwrap();

	let events = executor_context.drain_tx_events().unwrap();
	let event = events.get(0).unwrap().clone();
	let event = String::from_utf8(event.0).unwrap();

	assert_eq!(event, "{\"name\":\"MyEvent\",\"data\":{\"foo\":\"bar\"}}");
}

#[test]
fn test_vm_hw_hash() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#"{"data":[1]}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "hash", params, 0).unwrap();

	assert_eq!(
		result,
		r#""ee155ace9c40292074cb6aff8c9ccdd273c81648ff1149ef36bcea6ebb8a3e25""#.to_string()
	);
}

#[test]
fn test_vm_hw_address() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#"{"data":[1]}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "address", params, 0).unwrap();

	assert_eq!(
		result,
		r#""ca5d3fa0a6887285ef6aa85cb12960a2b6706e00""#.to_string()
	);
}

#[test]
fn test_vm_hw_validate_address() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#"{"address":"aa"}"#.as_bytes();
	let error = vm_execute(&context, Mode::Call, "validate_address", params, 0).unwrap_err();

	let expected_error: VMError = ContractError::InvalidAddress.into();

	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));

	let params = r#"{"address":"aa"}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "validate_address_ea", params, 0).unwrap();

	assert_eq!(
		result,
		r#""false: ContractError: InvalidAddress""#.to_string()
	);
}

#[test]
fn test_vm_hw_balance() {
	let (account1, _account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	endow(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3),
		executor_context,
	);

	let params = r#"{"address":"b4decd5a5f8f2ba708f8ced72eec89f44f3be96a"}"#.as_bytes();
	let result = vm_execute(&context, Mode::Call, "get_balance", params, 0).unwrap();

	assert_eq!(result, r#"1000"#.to_string());
}

#[test]
fn test_vm_hw_balance_transfer_success() {
	let _ = env_logger::try_init();

	let (account1, account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();
	let tx_hash = Some(Hash(vec![1]));
	endow(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let context = TestVMContext::new(
		tx_hash,
		contract_address.clone(),
		Some(account1.3.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"recipient":"{}", "value": 10}}"#, account2.3)
		.as_bytes()
		.to_vec();
	let _result = vm_execute(&context, Mode::Call, "balance_transfer", &params, 100);

	let events = executor_context
		.drain_tx_events()
		.unwrap()
		.into_iter()
		.map(|x| String::from_utf8(x.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("events: {:?}", events);
	assert_eq!(events.len(), 2);

	// shift to new context
	let tx_hash = Some(Hash(vec![1]));
	let context = Rc::new(TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3.clone()),
		executor_context,
	));

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
	let _ = env_logger::try_init();

	let (account1, account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();

	endow(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address.clone(),
		Some(account1.3.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"recipient":"{}", "value": 200}}"#, account2.3)
		.as_bytes()
		.to_vec();
	let _result = vm_execute(&context, Mode::Call, "balance_transfer", &params, 100);

	let events = executor_context
		.drain_tx_events()
		.unwrap()
		.into_iter()
		.map(|x| String::from_utf8(x.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("events: {:?}", events);
	assert_eq!(events.len(), 0);

	// shift to new context
	let tx_hash = Some(Hash(vec![2]));
	let context = Rc::new(TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3.clone()),
		executor_context,
	));

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
	let _ = env_logger::try_init();

	let (account1, account2) = base::test_accounts();

	let contract_address = Address(vec![1]);

	let executor_context = TestExecutorContext::new();

	endow(executor_context.clone(), vec![(account1.3.clone(), 1000)]).unwrap();

	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		tx_hash,
		contract_address.clone(),
		Some(account1.3.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"recipient":"{}", "value": 200}}"#, account2.3)
		.as_bytes()
		.to_vec();
	let result = vm_execute(&context, Mode::Call, "balance_transfer_ea", &params, 100);
	log::info!("result: {:?}", result);

	let events = executor_context
		.drain_tx_events()
		.unwrap()
		.into_iter()
		.map(|x| String::from_utf8(x.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("events: {:?}", events);
	assert_eq!(events.len(), 1);

	// shift to new context
	let tx_hash = Some(Hash(vec![1]));
	let context = Rc::new(TestVMContext::new(
		tx_hash,
		contract_address,
		Some(account1.3.clone()),
		executor_context,
	));

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
	context: &dyn VMContext,
	mode: Mode,
	method: &str,
	params: &[u8],
	pay_value: Balance,
) -> VMResult<String> {
	let code = get_code();
	base::vm_execute(code, context, mode, method, params, pay_value)
}

fn vm_validate(
	custom_code: Option<Vec<u8>>,
	mode: Mode,
	method: &str,
	params: &str,
	pay_value: Balance,
) -> VMResult<()> {
	let code = match custom_code {
		Some(code) => Cow::from(code),
		None => Cow::from(get_code()),
	};
	base::vm_validate(&*code, mode, method, params, pay_value)
}

fn get_code() -> &'static [u8] {
	let code = include_bytes!(
		"../contract-samples/hello-world/release/contract_samples_hello_world_bg.wasm"
	);
	code
}
