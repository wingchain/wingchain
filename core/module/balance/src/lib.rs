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

use executor_macro::{call, module};
use executor_primitives::{
	errors, Context, ContextEnv, EmptyParams, Module as ModuleT, StorageMap, Validator,
};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::CallResult;
use primitives::{codec, Address, Balance, Call, TransactionResult};

pub struct Module<C>
where
	C: Context,
{
	env: Rc<ContextEnv>,
	context: C,
	balance: StorageMap<Address, Balance, C>,
}

#[module]
impl<C: Context> Module<C> {
	const META_MODULE: bool = false;
	const STORAGE_KEY: &'static [u8] = b"balance";

	fn new(context: C) -> Self {
		Self {
			env: context.env(),
			context: context.clone(),
			balance: StorageMap::new::<Self>(context, b"balance"),
		}
	}
	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> CommonResult<CallResult<()>> {
		if self.env.number != 0 {
			return Ok(Err("not genesis".to_string()));
		}

		for (address, balance) in &params.endow {
			self.balance.set(address, balance)?;
		}
		Ok(Ok(()))
	}

	fn validate_init<V: Validator>(validator: &V, params: InitParams) -> CommonResult<()> {
		for (address, _) in params.endow {
			validator.validate_address(&address)?;
		}
		Ok(())
	}

	#[call]
	fn get_balance(
		&self,
		sender: Option<&Address>,
		_params: EmptyParams,
	) -> CommonResult<CallResult<Balance>> {
		let address = sender.expect("should be signed");
		let balance = self.balance.get(address)?;
		let balance = balance.unwrap_or(0);
		Ok(Ok(balance))
	}

	#[call(write = true)]
	fn transfer(
		&self,
		sender: Option<&Address>,
		params: TransferParams,
	) -> CommonResult<CallResult<()>> {
		let sender = sender.expect("should be signed");
		let recipient = &params.recipient;
		let value = params.value;

		if sender == recipient {
			return Ok(Err("should not transfer to oneself".to_string()));
		}

		let sender_balance = self.balance.get(sender)?.unwrap_or(0);
		if sender_balance < value {
			return Ok(Err("insufficient balance".to_string()));
		}
		let recipient_balance = self.balance.get(recipient)?.unwrap_or(0);

		let (sender_balance, overflow) = sender_balance.overflowing_sub(value);
		if overflow {
			return Ok(Err("u64 overflow".to_string()));
		}

		let (recipient_balance, overflow) = recipient_balance.overflowing_add(value);
		if overflow {
			return Ok(Err("u64 overflow".to_string()));
		}

		self.balance.set(sender, &sender_balance)?;
		self.balance.set(recipient, &recipient_balance)?;

		let event = TransferEvent {
			sender: sender.clone(),
			recipient: recipient.clone(),
			value,
		};

		self.context.emit_event(event)?;

		Ok(Ok(()))
	}

	fn validate_transfer<V: Validator>(validator: &V, params: TransferParams) -> CommonResult<()> {
		validator.validate_address(&params.recipient)?;
		Ok(())
	}
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct InitParams {
	pub endow: Vec<(Address, Balance)>,
}

#[derive(Encode, Decode)]
pub struct TransferParams {
	pub recipient: Address,
	pub value: Balance,
}

#[derive(Encode, Decode)]
pub struct TransferEvent {
	pub sender: Address,
	pub recipient: Address,
	pub value: Balance,
}
