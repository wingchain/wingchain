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
	block_interval: StorageValue<u64, Self>,
	heartbeat_interval: StorageValue<u64, Self>,
	election_timeout_min: StorageValue<u64, Self>,
	election_timeout_max: StorageValue<u64, Self>,
	authorities: StorageValue<Authorities, Self>,
}

#[module]
impl<C: Context, U: Util> Module<C, U> {
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"raft";

	fn new(context: C, util: U) -> Self {
		Self {
			env: context.env(),
			context: context.clone(),
			util,
			block_interval: StorageValue::new(context.clone(), b"block_interval"),
			heartbeat_interval: StorageValue::new(context.clone(), b"heartbeat_interval"),
			election_timeout_min: StorageValue::new(context.clone(), b"election_timeout_min"),
			election_timeout_max: StorageValue::new(context.clone(), b"election_timeout_max"),
			authorities: StorageValue::new(context, b"authorities"),
		}
	}

	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		if self.env.number != 0 {
			return Err("Not genesis".into());
		}
		self.block_interval.set(&params.block_interval)?;
		self.heartbeat_interval.set(&params.block_interval)?;
		self.election_timeout_min.set(&params.block_interval)?;
		self.election_timeout_max.set(&params.block_interval)?;
		self.authorities.set(&params.authorities)?;
		Ok(())
	}

	fn validate_init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		for (address, _) in &params.authorities.members {
			self.util.validate_address(address)?;
		}
		Ok(())
	}

	#[call]
	fn get_meta(&self, _sender: Option<&Address>, _params: EmptyParams) -> ModuleResult<Meta> {
		let block_interval = self.block_interval.get()?.ok_or("Unexpected none")?;
		let heartbeat_interval = self.heartbeat_interval.get()?.ok_or("Unexpected none")?;
		let election_timeout_min = self.election_timeout_min.get()?.ok_or("Unexpected none")?;
		let election_timeout_max = self.election_timeout_max.get()?.ok_or("Unexpected none")?;

		let meta = Meta {
			block_interval,
			heartbeat_interval,
			election_timeout_min,
			election_timeout_max,
		};
		Ok(meta)
	}

	#[call]
	fn get_authorities(
		&self,
		_sender: Option<&Address>,
		_params: EmptyParams,
	) -> ModuleResult<Authorities> {
		let authorities = self.authorities.get()?.ok_or("Unexpected none")?;
		Ok(authorities)
	}
}

#[derive(Encode, Decode, Debug, PartialEq, Deserialize)]
pub struct Authorities {
	pub threshold: u32,
	pub members: Vec<(Address, u32)>,
}

#[derive(Encode, Decode, Debug, PartialEq, Deserialize)]
pub struct InitParams {
	pub block_interval: u64,
	pub heartbeat_interval: u64,
	pub election_timeout_min: u64,
	pub election_timeout_max: u64,
	pub authorities: Authorities,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Meta {
	pub block_interval: u64,
	pub heartbeat_interval: u64,
	pub election_timeout_min: u64,
	pub election_timeout_max: u64,
}
