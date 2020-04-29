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

use serde::{Deserialize, Serialize};

use executor_macro::{call, module};
use executor_primitives::{
	errors::{self, execute_error_result},
	Context, ContextEnv, Module as ModuleT, StorageMap,
};
use primitives::errors::CommonResult;
use primitives::{codec, Address, Balance, Call};

pub struct Module<C>
where
	C: Context,
{
	env: Rc<ContextEnv>,
	balance: StorageMap<Address, Balance, C>,
}

#[module]
impl<C: Context> Module<C> {
	const META_MODULE: bool = false;
	const STORAGE_KEY: &'static [u8] = b"balance";

	fn new(context: C) -> Self {
		Self {
			env: context.env(),
			balance: StorageMap::new::<Self>(context, b"balance"),
		}
	}
	#[call(write = true)]
	fn init(
		&self,
		_sender: Option<&Address>,
		params: InitParams,
	) -> CommonResult<CommonResult<()>> {
		if self.env.number != 0 {
			return execute_error_result("not genesis");
		}

		for (address, balance) in &params.endow {
			self.balance.set(address, balance)?;
		}
		Ok(Ok(()))
	}

	#[call]
	fn get_balance(
		&self,
		sender: Option<&Address>,
		_params: EmptyParams,
	) -> CommonResult<CommonResult<()>> {
		let address = sender.expect("should be signed");
		let _balance = self.balance.get(address)?;
		// TODO
		Ok(Ok(()))
	}

	#[call(write = true)]
	fn transfer(
		&self,
		sender: Option<&Address>,
		params: TransferParams,
	) -> CommonResult<CommonResult<()>> {
		let sender = sender.expect("should be signed");
		let recipient = &params.recipient;
		let value = params.value;

		if sender == recipient {
			return execute_error_result("should not transfer to oneself");
		}

		let sender_balance = self.balance.get(sender)?.unwrap_or(0);
		if sender_balance < value {
			return execute_error_result("insufficient balance");
		}
		let recipient_balance = self.balance.get(recipient)?.unwrap_or(0);

		let (sender_balance, overflow) = sender_balance.overflowing_sub(value);
		if overflow {
			return execute_error_result("u64 overflow");
		}

		let (recipient_balance, overflow) = recipient_balance.overflowing_add(value);
		if overflow {
			return execute_error_result("u64 overflow");
		}

		self.balance.set(sender, &sender_balance)?;
		self.balance.set(recipient, &recipient_balance)?;

		Ok(Ok(()))
	}
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct InitParams {
	pub endow: Vec<(Address, Balance)>,
}

#[derive(Deserialize)]
pub struct EmptyParams;

#[derive(Serialize, Deserialize)]
pub struct TransferParams {
	pub recipient: Address,
	pub value: Balance,
}
