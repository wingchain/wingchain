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

use std::sync::Arc;

use executor_macro::{call, module};
use executor_primitives::{
	errors, Context, ContextEnv, EmptyParams, Module as ModuleT, ModuleResult, OpaqueModuleResult,
	StorageValue, Util,
};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, BlockNumber, Call};

pub struct Module<C, U>
where
	C: Context,
	U: Util,
{
	env: Arc<ContextEnv>,
	#[allow(dead_code)]
	util: U,
	chain_id: StorageValue<String, Self>,
	timestamp: StorageValue<u64, Self>,
	until_gap: StorageValue<BlockNumber, Self>,
}

#[module]
impl<C: Context, U: Util> Module<C, U> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"system";

	fn new(context: C, util: U) -> Self {
		Self {
			env: context.env(),
			util,
			chain_id: StorageValue::new(context.clone(), b"chain_id"),
			timestamp: StorageValue::new(context.clone(), b"timestamp"),
			until_gap: StorageValue::new(context, b"until_gap"),
		}
	}

	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		if self.env.number != 0 {
			return Err("Not genesis".into());
		}
		self.chain_id.set(&params.chain_id)?;
		self.timestamp.set(&params.timestamp)?;
		self.until_gap.set(&params.until_gap)?;
		Ok(())
	}

	#[call]
	fn get_meta(&self, _sender: Option<&Address>, _params: EmptyParams) -> ModuleResult<Meta> {
		let chain_id = self.chain_id.get()?.ok_or("Unexpected none")?;
		let timestamp = self.timestamp.get()?.ok_or("Unexpected none")?;
		let until_gap = self.until_gap.get()?.ok_or("Unexpected none")?;
		let meta = Meta {
			chain_id,
			timestamp,
			until_gap,
		};
		Ok(meta)
	}
}

pub type InitParams = Meta;

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Meta {
	pub chain_id: String,
	pub timestamp: u64,
	pub until_gap: BlockNumber,
}
