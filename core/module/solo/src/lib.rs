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
	StorageValue, Util,
};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Call};

pub struct Module<C, U>
where
	C: Context,
	U: Util,
{
	env: Rc<ContextEnv>,
	#[allow(dead_code)]
	util: U,
	block_interval: StorageValue<u64, Self>,
}

#[module]
impl<C: Context, U: Util> Module<C, U> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"solo";

	fn new(context: C, util: U) -> Self {
		Self {
			env: context.env(),
			util,
			block_interval: StorageValue::new(context.clone(), b"block_interval"),
		}
	}

	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		if self.env.number != 0 {
			return Err("Not genesis".into());
		}
		self.block_interval.set(&params.block_interval)?;
		Ok(())
	}

	#[call]
	fn get_meta(&self, _sender: Option<&Address>, _params: EmptyParams) -> ModuleResult<Meta> {
		let block_interval = self.block_interval.get()?;
		let block_interval = block_interval.ok_or("unexpected none")?;

		let meta = Meta { block_interval };
		Ok(meta)
	}
}

pub type InitParams = Meta;

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Meta {
	pub block_interval: u64,
}
