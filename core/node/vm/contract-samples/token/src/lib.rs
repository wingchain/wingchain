use serde::{Deserialize, Serialize};
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
	util: Util,
	name: StorageValue<String>,
	symbol: StorageValue<String>,
	decimals: StorageValue<u8>,
	total_supply: StorageValue<Balance>,
	balance: StorageMap<Balance>,
	allowance: StorageMap<Balance>,
}

#[contract]
impl Contract {
	fn new() -> ContractResult<Self> {
		let contract = Contract {
			context: Context::new()?,
			util: Util::new()?,
			name: StorageValue::new(b"name"),
			symbol: StorageValue::new(b"symbol"),
			decimals: StorageValue::new(b"decimals"),
			total_supply: StorageValue::new(b"totalSupply"),
			balance: StorageMap::new(b"balance"),
			allowance: StorageMap::new(b"allowance"),
		};
		Ok(contract)
	}

	#[init]
	fn init(&self, params: InitParams) -> ContractResult<()> {
		self.name.set(&params.name)?;
		self.symbol.set(&params.symbol)?;
		self.decimals.set(&params.decimals)?;
		self.total_supply.set(&params.total_supply)?;
		let sender_address = &self.context.contract_env()?.sender_address;
		self.balance.set(&sender_address.0, &params.total_supply)?;
		Ok(())
	}

	#[call]
	fn name(&self, _params: EmptyParams) -> ContractResult<String> {
		let name = self.name.get()?.expect("qed");
		Ok(name)
	}

	#[call]
	fn symbol(&self, _params: EmptyParams) -> ContractResult<String> {
		let symbol = self.symbol.get()?.expect("qed");
		Ok(symbol)
	}

	#[call]
	fn decimals(&self, _params: EmptyParams) -> ContractResult<u8> {
		let decimals = self.decimals.get()?.expect("qed");
		Ok(decimals)
	}

	#[call]
	fn total_supply(&self, _params: EmptyParams) -> ContractResult<Balance> {
		let total_supply = self.total_supply.get()?.expect("qed");
		Ok(total_supply)
	}

	#[call]
	fn balance(&self, params: BalanceParams) -> ContractResult<Balance> {
		let address = params.address;
		let balance = self.balance.get(&address.0)?.unwrap_or(0);
		Ok(balance)
	}

	#[call]
	fn transfer(&self, params: TransferParams) -> ContractResult<()> {
		let sender_address = &self.context.contract_env()?.sender_address;

		self.inner_transfer(sender_address, &params.recipient, params.value)
	}

	#[call]
	fn approve(&self, params: ApproveParams) -> ContractResult<()> {
		let owner_address = &self.context.contract_env()?.sender_address;

		self.inner_approve(owner_address, &params.spender, params.value)
	}

	#[call]
	fn allowance(&self, params: AllowanceParams) -> ContractResult<Balance> {
		let owner_address = params.owner;
		self.util.validate_address(&owner_address)?;

		let spender_address = params.spender;
		self.util.validate_address(&spender_address)?;

		let key = &[&owner_address.0[..], b"_", &spender_address.0[..]].concat();
		let result = self.allowance.get(key)?.unwrap_or(0);

		Ok(result)
	}

	#[call]
	fn transfer_from(&self, params: TransferFromParams) -> ContractResult<()> {
		let sender_address = params.sender;
		let recipient_address = params.recipient;

		let spender_address = &self.context.contract_env()?.sender_address;

		let key = &[&sender_address.0[..], b"_", &spender_address.0[..]].concat();
		let allowance = self.allowance.get(key)?.unwrap_or(0);

		let value = params.value;

		if allowance < value {
			return Err("exceed allowance".into());
		}

		self.inner_transfer(&sender_address, &recipient_address, value)?;

		let (allowance, overflow) = allowance.overflowing_sub(value);
		if overflow {
			return Err("u64 overflow".into());
		}

		self.inner_approve(&sender_address, spender_address, allowance)?;

		Ok(())
	}

	fn inner_transfer(
		&self,
		sender_address: &Address,
		recipient_address: &Address,
		value: Balance,
	) -> ContractResult<()> {
		self.util.validate_address(&sender_address)?;
		self.util.validate_address(&recipient_address)?;

		let sender_balance = self.balance.get(&sender_address.0)?.unwrap_or(0);
		let recipient_balance = self.balance.get(&recipient_address.0)?.unwrap_or(0);

		let (sender_balance, overflow) = sender_balance.overflowing_sub(value);
		if overflow {
			return Err("u64 overflow".into());
		}

		let (recipient_balance, overflow) = recipient_balance.overflowing_add(value);
		if overflow {
			return Err("u64 overflow".into());
		}

		self.balance.set(&sender_address.0, &sender_balance)?;
		self.balance.set(&recipient_address.0, &recipient_balance)?;

		self.context.emit_event(
			"Transfer".to_string(),
			TransferEvent {
				sender: sender_address.clone(),
				recipient: recipient_address.clone(),
				value,
			},
		)?;
		Ok(())
	}

	fn inner_approve(
		&self,
		owner_address: &Address,
		spender_address: &Address,
		value: Balance,
	) -> ContractResult<()> {
		self.util.validate_address(&owner_address)?;
		self.util.validate_address(&spender_address)?;

		let key = &[&owner_address.0[..], b"_", &spender_address.0[..]].concat();
		self.allowance.set(key, &value)?;

		self.context.emit_event(
			"Approval".to_string(),
			ApprovalEvent {
				owner: owner_address.clone(),
				spender: spender_address.clone(),
				value,
			},
		)?;

		Ok(())
	}
}

#[derive(Deserialize)]
struct InitParams {
	name: String,
	symbol: String,
	decimals: u8,
	total_supply: Balance,
}

#[derive(Deserialize)]
struct BalanceParams {
	address: Address,
}

#[derive(Deserialize)]
struct TransferParams {
	recipient: Address,
	value: Balance,
}

#[derive(Deserialize)]
struct ApproveParams {
	spender: Address,
	value: Balance,
}

#[derive(Deserialize)]
struct AllowanceParams {
	owner: Address,
	spender: Address,
}

#[derive(Deserialize)]
struct TransferFromParams {
	sender: Address,
	recipient: Address,
	value: Balance,
}

#[derive(Serialize)]
struct TransferEvent {
	sender: Address,
	recipient: Address,
	value: Balance,
}

#[derive(Serialize)]
struct ApprovalEvent {
	owner: Address,
	spender: Address,
	value: Balance,
}
