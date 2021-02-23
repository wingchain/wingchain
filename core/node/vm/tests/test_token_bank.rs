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

use base::{TestExecutorContext, TestVMContext};
use node_vm::errors::{ContractError, VMError, VMResult};
use node_vm::{Mode, VMConfig, VMContext};
use primitives::{Address, Balance, Hash};

use crate::base::ExecutorContext;

mod base;

#[test]
fn test_vm_tb_success() {
	let _ = env_logger::try_init();

	let test_accounts = base::test_accounts();
	let (account1, _account2) = (&test_accounts[0], &test_accounts[1]);

	let executor_context = TestExecutorContext::new();

	let token_contract_address = Address(vec![1; 20]);

	base::create_contract(
		&token_contract_address,
		&account1.address,
		get_token_code(),
		executor_context.clone(),
		"init",
		r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#
			.as_bytes(),
		0,
	)
	.unwrap();
	let token_bank_contract_address = Address(vec![2; 20]);

	// init token bank contract
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = format!(
		r#"{{"token_contract_address":"{}"}}"#,
		token_contract_address
	)
	.as_bytes()
	.to_vec();
	let result = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Init,
		"init",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// approve in token contract
	let tx_hash = Some(Hash(vec![2]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(
		r#"{{"spender":"{}","value":100}}"#,
		token_bank_contract_address
	)
	.as_bytes()
	.to_vec();

	let result = vm_execute(
		get_token_code(),
		&context,
		Mode::Call,
		"approve",
		&params,
		0,
	)
	.unwrap();

	executor_context.drain_tx_events().unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// deposit in token bank contract
	let tx_hash = Some(Hash(vec![3]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"value":90}}"#).as_bytes().to_vec();

	let result = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Call,
		"deposit",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check bank balance after depositing
	let tx_hash = Some(Hash(vec![4]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = r#""#.as_bytes();

	let result = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Call,
		"balance",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"90"#.to_string());

	// check token balance after depositing
	let tx_hash = Some(Hash(vec![5]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"address":"{}"}}"#, account1.address)
		.as_bytes()
		.to_vec();

	let result = vm_execute(
		get_token_code(),
		&context,
		Mode::Call,
		"balance",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"2099999999999910"#.to_string());

	let tx_hash = Some(Hash(vec![6]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"address":"{}"}}"#, token_bank_contract_address.clone())
		.as_bytes()
		.to_vec();

	let result = vm_execute(
		get_token_code(),
		&context,
		Mode::Call,
		"balance",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"90"#.to_string());

	let events = executor_context
		.drain_tx_events()
		.unwrap()
		.into_iter()
		.map(|x| String::from_utf8(x.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("events: {:?}", events);
	assert_eq!(events.len(), 2);

	// withdraw in token bank contract
	let tx_hash = Some(Hash(vec![7]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"value":50}}"#).as_bytes().to_vec();

	let result = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Call,
		"withdraw",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check bank balance after withdrawing
	let tx_hash = Some(Hash(vec![8]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = r#""#.as_bytes();

	let result = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Call,
		"balance",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"40"#.to_string());

	// check token balance after withdrawing
	let tx_hash = Some(Hash(vec![9]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"address":"{}"}}"#, account1.address)
		.as_bytes()
		.to_vec();

	let result = vm_execute(
		get_token_code(),
		&context,
		Mode::Call,
		"balance",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"2099999999999960"#.to_string());

	let tx_hash = Some(Hash(vec![10]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"address":"{}"}}"#, token_bank_contract_address.clone())
		.as_bytes()
		.to_vec();

	let result = vm_execute(
		get_token_code(),
		&context,
		Mode::Call,
		"balance",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"40"#.to_string());

	let events = executor_context
		.drain_tx_events()
		.unwrap()
		.into_iter()
		.map(|x| String::from_utf8(x.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("events: {:?}", events);
	assert_eq!(events.len(), 1);
}

#[test]
fn test_vm_tb_failed() {
	let _ = env_logger::try_init();

	let test_accounts = base::test_accounts();
	let (account1, _account2) = (&test_accounts[0], &test_accounts[1]);

	let executor_context = TestExecutorContext::new();

	let token_contract_address = Address(vec![1; 20]);

	base::create_contract(
		&token_contract_address,
		&account1.address,
		get_token_code(),
		executor_context.clone(),
		"init",
		r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#
			.as_bytes(),
		0,
	)
	.unwrap();
	let token_bank_contract_address = Address(vec![2; 20]);

	// init token bank contract
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = format!(
		r#"{{"token_contract_address":"{}"}}"#,
		token_contract_address
	)
	.as_bytes()
	.to_vec();
	let result = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Init,
		"init",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// approve in token contract
	let tx_hash = Some(Hash(vec![2]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(
		r#"{{"spender":"{}","value":100}}"#,
		token_bank_contract_address
	)
	.as_bytes()
	.to_vec();

	let result = vm_execute(
		get_token_code(),
		&context,
		Mode::Call,
		"approve",
		&params,
		0,
	)
	.unwrap();

	executor_context.drain_tx_events().unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// deposit in token bank contract
	let tx_hash = Some(Hash(vec![3]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"value":110}}"#).as_bytes().to_vec();

	let error = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Call,
		"deposit",
		&params,
		0,
	)
	.unwrap_err();
	let expected_error: VMError = ContractError::User {
		msg: "Exceed allowance".to_string(),
	}
	.into();
	assert_eq!(format!("{:?}", error), format!("{:?}", expected_error));
}

#[test]
fn test_vm_tb_ea() {
	let _ = env_logger::try_init();

	let test_accounts = base::test_accounts();
	let (account1, _account2) = (&test_accounts[0], &test_accounts[1]);

	let executor_context = TestExecutorContext::new();

	let token_contract_address = Address(vec![1; 20]);

	base::create_contract(
		&token_contract_address,
		&account1.address,
		get_token_code(),
		executor_context.clone(),
		"init",
		r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#
			.as_bytes(),
		0,
	)
	.unwrap();
	let token_bank_contract_address = Address(vec![2; 20]);

	// init token bank contract
	let tx_hash = Some(Hash(vec![1]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = format!(
		r#"{{"token_contract_address":"{}"}}"#,
		token_contract_address
	)
	.as_bytes()
	.to_vec();
	let result = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Init,
		"init",
		&params,
		0,
	)
	.unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// approve in token contract
	let tx_hash = Some(Hash(vec![2]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(
		r#"{{"spender":"{}","value":100}}"#,
		token_bank_contract_address
	)
	.as_bytes()
	.to_vec();

	let result = vm_execute(
		get_token_code(),
		&context,
		Mode::Call,
		"approve",
		&params,
		0,
	)
	.unwrap();

	executor_context.drain_tx_events().unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// deposit in token bank contract
	let tx_hash = Some(Hash(vec![3]));
	let context = TestVMContext::new(
		VMConfig::default(),
		tx_hash,
		Some(token_bank_contract_address.clone()),
		Some(account1.address.clone()),
		executor_context.clone(),
	);

	let params = &format!(r#"{{"value":110}}"#).as_bytes().to_vec();

	let success = vm_execute(
		get_token_bank_code(),
		&context,
		Mode::Call,
		"deposit_ea",
		&params,
		0,
	)
	.unwrap();
	assert_eq!(
		success,
		r#""false: ContractError: Exceed allowance""#.to_string()
	);
}

fn vm_execute(
	code: &[u8],
	context: &dyn VMContext,
	mode: Mode,
	method: &str,
	params: &[u8],
	pay_value: Balance,
) -> VMResult<String> {
	base::vm_execute(code, context, mode, method, params, pay_value)
}

fn get_token_bank_code() -> &'static [u8] {
	let code = include_bytes!(
		"../contract-samples/token-bank/release/contract_samples_token_bank_bg.wasm"
	);
	code
}

fn get_token_code() -> &'static [u8] {
	let code = include_bytes!("../contract-samples/token/release/contract_samples_token_bg.wasm");
	code
}
