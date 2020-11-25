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
use node_vm::errors::VMResult;
use node_vm::{Mode, VMContext};
use primitives::Balance;

mod base;

#[test]
fn test_vm_token_transfer() {
	let (account1, account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();

	let context = TestVMContext::new(account1.3.clone(), executor_context);

	// init
	let params =
		r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#;
	let result = vm_execute(&context, Mode::Init, "init", params, 0).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// get name
	let params = r#""#;
	let result = vm_execute(&context, Mode::Call, "name", params, 0).unwrap();

	assert_eq!(result, r#""Bitcoin""#.to_string());

	// get symbol
	let params = r#""#;
	let result = vm_execute(&context, Mode::Call, "symbol", params, 0).unwrap();

	assert_eq!(result, r#""BTC""#.to_string());

	// get decimals
	let params = r#""#;
	let result = vm_execute(&context, Mode::Call, "decimals", params, 0).unwrap();

	assert_eq!(result, r#"8"#.to_string());

	// get total supply
	let params = r#""#;
	let result = vm_execute(&context, Mode::Call, "total_supply", params, 0).unwrap();

	assert_eq!(result, r#"2100000000000000"#.to_string());

	// get balance
	let params = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(&context, Mode::Call, "balance", &params, 0).unwrap();

	assert_eq!(result, r#"2100000000000000"#.to_string());

	// transfer
	let params = format!(
		r#"{{"recipient":"{}","value":100000000000000}}"#,
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(&context, Mode::Call, "transfer", &params, 0).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check balance after transferring
	let params = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(&context, Mode::Call, "balance", &params, 0).unwrap();

	assert_eq!(result, r#"2000000000000000"#.to_string());

	let params = format!(r#"{{"address":"{}"}}"#, hex::encode((account2.3).0.clone()));

	let result = vm_execute(&context, Mode::Call, "balance", &params, 0).unwrap();

	assert_eq!(result, r#"100000000000000"#.to_string());
}

#[test]
fn test_vm_token_transfer_from() {
	let (account1, account2) = base::test_accounts();

	let executor_context = TestExecutorContext::new();

	let context = TestVMContext::new(account1.3.clone(), executor_context.clone());

	// init
	let params =
		r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#;
	let result = vm_execute(&context, Mode::Init, "init", params, 0).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// get balance
	let params = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(&context, Mode::Call, "balance", &params, 0).unwrap();

	assert_eq!(result, r#"2100000000000000"#.to_string());

	// approve
	let params = format!(
		r#"{{"spender":"{}","value":100000000000000}}"#,
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(&context, Mode::Call, "approve", &params, 0).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check allowance after approving
	let params = format!(
		r#"{{"owner":"{}","spender":"{}"}}"#,
		hex::encode((account1.3).0.clone()),
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(&context, Mode::Call, "allowance", &params, 0).unwrap();

	assert_eq!(result, r#"100000000000000"#.to_string());

	// shift to account2
	let context = TestVMContext::new(account2.3.clone(), executor_context);

	// transfer from
	let params = format!(
		r#"{{"sender":"{}","recipient":"{}","value":100000000}}"#,
		hex::encode((account1.3).0.clone()),
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(&context, Mode::Call, "transfer_from", &params, 0).unwrap();

	assert_eq!(result, r#"null"#.to_string());

	// check allowance after transfer from
	let params = format!(
		r#"{{"owner":"{}","spender":"{}"}}"#,
		hex::encode((account1.3).0.clone()),
		hex::encode((account2.3).0.clone())
	);

	let result = vm_execute(&context, Mode::Call, "allowance", &params, 0).unwrap();

	assert_eq!(result, r#"99999900000000"#.to_string());

	// check sender balance
	let params = format!(r#"{{"address":"{}"}}"#, hex::encode((account1.3).0.clone()));

	let result = vm_execute(&context, Mode::Call, "balance", &params, 0).unwrap();

	assert_eq!(result, r#"2099999900000000"#.to_string());

	// check recipient balance
	let params = format!(r#"{{"address":"{}"}}"#, hex::encode((account2.3).0.clone()));

	let result = vm_execute(&context, Mode::Call, "balance", &params, 0).unwrap();

	assert_eq!(result, r#"100000000"#.to_string());
}

fn vm_execute(
	context: &dyn VMContext,
	mode: Mode,
	method: &str,
	params: &str,
	pay_value: Balance,
) -> VMResult<String> {
	let code = get_code();
	base::vm_execute(code, context, mode, method, params, pay_value)
}

fn get_code() -> &'static [u8] {
	let code = include_bytes!("../contract-samples/token/pkg/contract_samples_token_bg.wasm");
	code
}
