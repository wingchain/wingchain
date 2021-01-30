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
use primitives::{codec, Address, Call};
use serde::Deserialize;

pub struct Module<C, U>
where
	C: Context,
	U: Util,
{
	env: Arc<ContextEnv>,
	#[allow(dead_code)]
	context: C,
	util: U,
	block_interval: StorageValue<Option<u64>, Self>,
	admin: StorageValue<Admin, Self>,
	authority: StorageValue<Address, Self>,
}

#[module]
impl<C: Context, U: Util> Module<C, U> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"poa";

	fn new(context: C, util: U) -> Self {
		Self {
			env: context.env(),
			context: context.clone(),
			util,
			block_interval: StorageValue::new(context.clone(), b"block_interval"),
			admin: StorageValue::new(context.clone(), b"admin"),
			authority: StorageValue::new(context, b"authority"),
		}
	}

	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		if self.env.number != 0 {
			return Err("Not genesis".into());
		}
		self.block_interval.set(&params.block_interval)?;
		self.admin.set(&params.admin)?;
		self.authority.set(&params.authority)?;
		Ok(())
	}

	fn validate_init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		for (address, _) in &params.admin.members {
			self.util.validate_address(address)?;
		}
		self.util.validate_address(&params.authority)?;
		Ok(())
	}

	#[call]
	fn get_meta(&self, _sender: Option<&Address>, _params: EmptyParams) -> ModuleResult<Meta> {
		let block_interval = self.block_interval.get()?;
		let block_interval = block_interval.ok_or("Unexpected none")?;

		let meta = Meta { block_interval };
		Ok(meta)
	}

	#[call]
	fn get_authority(
		&self,
		_sender: Option<&Address>,
		_params: EmptyParams,
	) -> ModuleResult<Address> {
		let authority = self.authority.get()?;
		let authority = authority.ok_or("Unexpected none")?;

		Ok(authority)
	}
}

#[derive(Encode, Decode, Debug, PartialEq, Deserialize)]
pub struct Admin {
	pub threshold: u32,
	pub members: Vec<(Address, u32)>,
}

#[derive(Encode, Decode, Debug, PartialEq, Deserialize)]
pub struct InitParams {
	pub block_interval: Option<u64>,
	pub admin: Admin,
	pub authority: Address,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Meta {
	pub block_interval: Option<u64>,
}
