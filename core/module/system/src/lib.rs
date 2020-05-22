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
	CallResult, Context, ContextEnv, Module as ModuleT, StorageValue, Validator, EmptyParams
};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::{codec, Address, Call, BlockNumber};

pub struct Module<C>
where
	C: Context,
{
	env: Rc<ContextEnv>,
	chain_id: StorageValue<String, C>,
	timestamp: StorageValue<u32, C>,
	until_gap: StorageValue<BlockNumber, C>,
}

#[module]
impl<C: Context> Module<C> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"system";

	fn new(context: C) -> Self {
		Self {
			env: context.env(),
			chain_id: StorageValue::new::<Self>(context.clone(), b"chain_id"),
			timestamp: StorageValue::new::<Self>(context.clone(), b"timestamp"),
			until_gap: StorageValue::new::<Self>(context, b"until_gap"),
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
		self.until_gap.set(&params.until_gap)?;
		Ok(Ok(()))
	}

	#[call]
	fn get_meta(
		&self,
		_sender: Option<&Address>,
		_params: EmptyParams,
	) -> CommonResult<CommonResult<Meta>> {
		let chain_id = match self.chain_id.get()? {
			Some(chain_id) => chain_id,
			None => return execute_error_result("unexpected none"),
		};
		let timestamp = match self.timestamp.get()? {
			Some(timestamp) => timestamp,
			None => return execute_error_result("unexpected none"),
		};
		let until_gap = match self.until_gap.get()? {
			Some(until_gap) => until_gap,
			None => return execute_error_result("unexpected none"),
		};
		let meta = Meta{
			chain_id,
			timestamp,
			until_gap
		};
		Ok(Ok(meta))
	}
}

pub type InitParams = Meta;

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Meta {
	pub chain_id: String,
	pub timestamp: u32,
	pub until_gap: BlockNumber,
}
