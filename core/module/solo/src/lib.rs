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
	errors, Context, ContextEnv, EmptyParams, Module as ModuleT, StorageValue, Validator,
};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::CallResult;
use primitives::{codec, Address, Call, TransactionResult};

pub struct Module<C>
where
	C: Context,
{
	env: Rc<ContextEnv>,
	block_interval: StorageValue<u64, C>,
}

#[module]
impl<C: Context> Module<C> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"solo";

	fn new(context: C) -> Self {
		Self {
			env: context.env(),
			block_interval: StorageValue::new::<Self>(context.clone(), b"block_interval"),
		}
	}

	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> CommonResult<CallResult<()>> {
		if self.env.number != 0 {
			return Ok(Err("not genesis".to_string()));
		}
		self.block_interval.set(&params.block_interval)?;
		Ok(Ok(()))
	}

	#[call]
	fn get_meta(
		&self,
		_sender: Option<&Address>,
		_params: EmptyParams,
	) -> CommonResult<CallResult<Meta>> {
		let block_interval = match self.block_interval.get()? {
			Some(block_interval) => block_interval,
			None => return Ok(Err("unexpected none".to_string())),
		};
		let meta = Meta { block_interval };
		Ok(Ok(meta))
	}
}

pub type InitParams = Meta;

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Meta {
	pub block_interval: u64,
}
