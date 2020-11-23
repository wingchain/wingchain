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

use base::{TestExecutorContext, TestVMContext};
use node_vm::errors::VMResult;
use node_vm::{Mode, VMContext};

mod base;

#[test]
fn test_vm_token_transfer() {
	let (account1, account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();

	let context = Rc::new(TestVMContext::new(account1.3.clone(), 0, executor_context));

	// init
	let input = r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#;
	let result = vm_execute(context.clone(), Mode::Init, "init", input).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// get name
	let input = r#""#;
	let result = vm_execute(context.clone(), Mode::Call, "name", input).unwrap();

	assert_eq!(result, r#""Bitcoin""#.to_string());

	// get symbol
	let input = r#""#;
	let result = vm_execute(context.clone(), Mode::Call, "symbol", input).unwrap();

	assert_eq!(result, r#""BTC""#.to_string());

	// get decimals
	let input = r#""#;
	let result = vm_execute(context.clone(), Mode::Call, "decimals", input).unwrap();

	assert_eq!(result, r#"8"#.to_string());

	// get total supply
	let input = r#""#;
	let result = vm_execute(context.clone(), Mode::Call, "total_supply", input).unwrap();

	assert_eq!(result, r#"2100000000000000"#.to_string());

	// get balance
	let input = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(context.clone(), Mode::Call, "balance", &input).unwrap();

	assert_eq!(result, r#"2100000000000000"#.to_string());

	// transfer
	let input = format!(
		r#"{{"recipient":"{}","value":100000000000000}}"#,
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(context.clone(), Mode::Call, "transfer", &input).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check balance after transferring
	let input = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(context.clone(), Mode::Call, "balance", &input).unwrap();

	assert_eq!(result, r#"2000000000000000"#.to_string());

	let input = format!(r#"{{"address":"{}"}}"#, hex::encode((account2.3).0.clone()));

	let result = vm_execute(context.clone(), Mode::Call, "balance", &input).unwrap();

	assert_eq!(result, r#"100000000000000"#.to_string());
}

#[test]
fn test_vm_token_transfer_from() {
	let (account1, account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();

	let context = Rc::new(TestVMContext::new(
		account1.3.clone(),
		0,
		executor_context.clone(),
	));

	// init
	let input = r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#;
	let result = vm_execute(context.clone(), Mode::Init, "init", input).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// get balance
	let input = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(context.clone(), Mode::Call, "balance", &input).unwrap();

	assert_eq!(result, r#"2100000000000000"#.to_string());

	// approve
	let input = format!(
		r#"{{"spender":"{}","value":100000000000000}}"#,
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(context.clone(), Mode::Call, "approve", &input).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check allowance after approving
	let input = format!(
		r#"{{"owner":"{}","spender":"{}"}}"#,
		hex::encode((account1.3).0.clone()),
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(context.clone(), Mode::Call, "allowance", &input).unwrap();

	assert_eq!(result, r#"100000000000000"#.to_string());

	// apply
	context
		.payload_apply(context.payload_drain_buffer().unwrap())
		.unwrap();

	// shift to account2
	let context = Rc::new(TestVMContext::new(account2.3.clone(), 0, executor_context));

	// transfer from
	let input = format!(
		r#"{{"sender":"{}","recipient":"{}","value":100000000}}"#,
		hex::encode((account1.3).0.clone()),
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(context.clone(), Mode::Call, "transfer_from", &input).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check allowance after transfer from
	let input = format!(
		r#"{{"owner":"{}","spender":"{}"}}"#,
		hex::encode((account1.3).0.clone()),
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(context.clone(), Mode::Call, "allowance", &input).unwrap();

	assert_eq!(result, r#"99999900000000"#.to_string());

	// check sender balance
	let input = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(context.clone(), Mode::Call, "balance", &input).unwrap();

	assert_eq!(result, r#"2099999900000000"#.to_string());

	// check recipient balance
	let input = format!(r#"{{"address":"{}"}}"#, hex::encode((account2.3).0.clone()));

	let result = vm_execute(context.clone(), Mode::Call, "balance", &input).unwrap();

	assert_eq!(result, r#"100000000"#.to_string());
}

fn vm_execute(
	context: Rc<dyn VMContext>,
	mode: Mode,
	method: &str,
	input: &str,
) -> VMResult<String> {
	let code = include_bytes!("../contract-samples/token/pkg/contract_samples_token_bg.wasm");
	let code = &code[..];
	base::vm_execute(code, context, mode, method, input)
}
