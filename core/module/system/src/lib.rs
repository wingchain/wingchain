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
	errors::{self, execute_error_result},
	Context, ContextEnv, Module as ModuleT, StorageValue,
};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::{codec, Address, Call};

pub struct Module<C>
where
	C: Context,
{
	env: Rc<ContextEnv>,
	chain_id: StorageValue<String, C>,
	timestamp: StorageValue<u32, C>,
}

#[module]
impl<C: Context> Module<C> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"system";

	fn new(context: C) -> Self {
		Self {
			env: context.env(),
			chain_id: StorageValue::new::<Self>(context.clone(), b"chain_id"),
			timestamp: StorageValue::new::<Self>(context, b"timestamp"),
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
		self.chain_id.set(&params.chain_id)?;
		self.timestamp.set(&params.timestamp)?;
		Ok(Ok(()))
	}
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct InitParams {
	pub chain_id: String,
	pub timestamp: u32,
}
