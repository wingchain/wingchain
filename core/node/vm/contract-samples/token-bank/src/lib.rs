use serde::Deserialize;
use wasm_bindgen::prelude::*;

use sdk::{
	call, contract, import, init, serde_json, Address, Balance, Context, ContractError,
	ContractResult, EmptyParams, StorageMap, StorageValue, Util,
};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

struct Contract {
	context: Context,
	#[allow(dead_code)]
	util: Util,
	token_contract_address: StorageValue<Address>,
	balance: StorageMap<Balance>,
}

#[contract]
impl Contract {
	fn new() -> ContractResult<Self> {
		let contract = Contract {
			context: Context::new()?,
			util: Util::new()?,
			token_contract_address: StorageValue::new(b"token_contract_address"),
			balance: StorageMap::new(b"balance"),
		};
		Ok(contract)
	}

	#[init]
	fn init(&self, params: InitParams) -> ContractResult<()> {
		self.token_contract_address
			.set(&params.token_contract_address)?;
		Ok(())
	}

	#[call]
	fn balance(&self, _params: EmptyParams) -> ContractResult<Balance> {
		let sender_address = &self.context.contract_env()?.sender_address;
		let sender_address = sender_address.as_ref().ok_or(ContractError::Unsigned)?;
		let balance = self.balance.get(&sender_address.0)?.unwrap_or(0);
		Ok(balance)
	}

	#[call]
	fn deposit(&self, params: DepositParams) -> ContractResult<()> {
		let sender_address = &self.context.contract_env()?.sender_address;
		let sender_address = sender_address.as_ref().ok_or(ContractError::Unsigned)?;

		let this_contract_address = &self.context.contract_env()?.contract_address;

		let token_contract_address = self.token_contract_address.get()?.expect("qed");

		let value = params.value;

		let params = format!(
			r#"{{"sender":"{}","recipient":"{}","value":{}}}"#,
			sender_address, this_contract_address, value
		)
		.as_bytes()
		.to_vec();
		let _result =
			self.context
				.contract_execute(&token_contract_address, "transfer_from", &params, 0)?;

		let balance = self.balance.get(&sender_address.0)?.unwrap_or(0);
		let (balance, overflow) = balance.overflowing_add(value);
		if overflow {
			return Err("U64 overflow".into());
		}
		self.balance.set(&sender_address.0, &balance)?;
		Ok(())
	}

	#[call]
	fn withdraw(&self, params: WithdrawParams) -> ContractResult<()> {
		let sender_address = &self.context.contract_env()?.sender_address;
		let sender_address = sender_address.as_ref().ok_or(ContractError::Unsigned)?;

		let token_contract_address = self.token_contract_address.get()?.expect("qed");

		let value = params.value;

		let balance = self.balance.get(&sender_address.0)?.unwrap_or(0);

		if value > balance {
			return Err("Insufficient balance".into());
		}

		let params = format!(r#"{{"recipient":"{}","value":{}}}"#, sender_address, value)
			.as_bytes()
			.to_vec();
		let _result =
			self.context
				.contract_execute(&token_contract_address, "transfer", &params, 0)?;

		let (balance, overflow) = balance.overflowing_sub(value);
		if overflow {
			return Err("U64 overflow".into());
		}
		self.balance.set(&sender_address.0, &balance)?;
		Ok(())
	}

	#[call]
	fn deposit_ea(&self, params: DepositParams) -> ContractResult<String> {
		let sender_address = &self.context.contract_env()?.sender_address;
		let sender_address = sender_address.as_ref().ok_or(ContractError::Unsigned)?;

		let this_contract_address = &self.context.contract_env()?.contract_address;

		let token_contract_address = self.token_contract_address.get()?.expect("qed");

		let value = params.value;

		let params = format!(
			r#"{{"sender":"{}","recipient":"{}","value":{}}}"#,
			sender_address, this_contract_address, value
		)
		.as_bytes()
		.to_vec();
		let result =
			self.context
				.contract_execute_ea(&token_contract_address, "transfer_from", &params, 0);

		if let Err(e) = result {
			return Ok(format!("false: {}", e));
		}

		let balance = self.balance.get(&sender_address.0)?.unwrap_or(0);
		let (balance, overflow) = balance.overflowing_add(value);
		if overflow {
			return Err("U64 overflow".into());
		}
		self.balance.set(&sender_address.0, &balance)?;
		Ok("true".to_string())
	}

	#[call]
	fn withdraw_ea(&self, params: WithdrawParams) -> ContractResult<String> {
		let sender_address = &self.context.contract_env()?.sender_address;
		let sender_address = sender_address.as_ref().ok_or(ContractError::Unsigned)?;

		let token_contract_address = self.token_contract_address.get()?.expect("qed");

		let value = params.value;

		let balance = self.balance.get(&sender_address.0)?.unwrap_or(0);

		if value > balance {
			return Err("Insufficient balance".into());
		}

		let params = format!(r#"{{"recipient":"{}","value":{}}}"#, sender_address, value)
			.as_bytes()
			.to_vec();
		let result =
			self.context
				.contract_execute_ea(&token_contract_address, "transfer", &params, 0);
		if let Err(e) = result {
			return Ok(format!("false: {}", e));
		}

		let (balance, overflow) = balance.overflowing_sub(value);
		if overflow {
			return Err("U64 overflow".into());
		}
		self.balance.set(&sender_address.0, &balance)?;
		Ok("true".to_string())
	}
}

#[derive(Deserialize)]
struct InitParams {
	token_contract_address: Address,
}

#[derive(Deserialize)]
struct DepositParams {
	value: Balance,
}

#[derive(Deserialize)]
struct WithdrawParams {
	value: Balance,
}
