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
	errors, Context, ContextEnv, EmptyParams, Module as ModuleT, ModuleResult, OpaqueModuleResult,
	StorageMap, Util,
};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Balance, Call};

pub struct Module<C, U>
where
	C: Context,
	U: Util,
{
	env: Rc<ContextEnv>,
	context: C,
	#[allow(dead_code)]
	util: U,
	balance: StorageMap<Address, Balance, Self>,
}

#[module]
impl<C: Context, U: Util> Module<C, U> {
	const META_MODULE: bool = false;
	const STORAGE_KEY: &'static [u8] = b"balance";

	fn new(context: C, util: U) -> Self {
		Self {
			env: context.env(),
			context: context.clone(),
			util,
			balance: StorageMap::new(context, b"balance"),
		}
	}
	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		if self.env.number != 0 {
			return Err("not genesis".into());
		}

		for (address, balance) in &params.endow {
			self.balance.set(address, balance)?;
		}
		Ok(())
	}

	fn validate_init(util: &U, params: InitParams) -> ModuleResult<()> {
		for (address, _) in params.endow {
			util.validate_address(&address)?;
		}
		Ok(())
	}

	#[call]
	pub fn get_balance(
		&self,
		sender: Option<&Address>,
		_params: EmptyParams,
	) -> ModuleResult<Balance> {
		let sender = sender.ok_or("should be signed")?;
		let balance = self.balance.get(sender)?;
		let balance = balance.unwrap_or(0);
		Ok(balance)
	}

	#[call(write = true)]
	pub fn transfer(&self, sender: Option<&Address>, params: TransferParams) -> ModuleResult<()> {
		let sender = sender.ok_or("should be signed")?;
		let recipient = &params.recipient;
		let value = params.value;

		if sender == recipient {
			return Err("should not transfer to oneself".into());
		}

		let sender_balance = self.balance.get(sender)?.unwrap_or(0);
		if sender_balance < value {
			return Err("insufficient balance".into());
		}
		let recipient_balance = self.balance.get(recipient)?.unwrap_or(0);

		let (sender_balance, overflow) = sender_balance.overflowing_sub(value);
		if overflow {
			return Err("u64 overflow".into());
		}

		let (recipient_balance, overflow) = recipient_balance.overflowing_add(value);
		if overflow {
			return Err("u64 overflow".into());
		}

		self.balance.set(sender, &sender_balance)?;
		self.balance.set(recipient, &recipient_balance)?;

		let event = TransferEvent::Transferred(Transferred {
			sender: sender.clone(),
			recipient: recipient.clone(),
			value,
		});

		self.context.emit_event(event)?;

		Ok(())
	}

	pub fn validate_transfer(util: &U, params: TransferParams) -> ModuleResult<()> {
		util.validate_address(&params.recipient)?;
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
pub enum TransferEvent {
	Transferred(Transferred),
}

#[derive(Encode, Decode)]
pub struct Transferred {
	pub sender: Address,
	pub recipient: Address,
	pub value: Balance,
}
