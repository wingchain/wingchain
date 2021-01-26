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
	errors, errors::ApplicationError, Context, ContextEnv, EmptyParams, Module as ModuleT,
	ModuleResult, OpaqueModuleResult, StorageValue, Util,
};
use primitives::codec::{Decode, Encode};
use primitives::{codec, Address, Call, Event};
use serde::{Deserialize, Serialize};

pub struct Module<C, U>
where
	C: Context,
	U: Util,
{
	env: Arc<ContextEnv>,
	context: C,
	util: U,
	block_interval: StorageValue<Option<u64>, Self>,
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
			authority: StorageValue::new(context, b"authority"),
		}
	}

	#[call(write = true)]
	fn init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
		if self.env.number != 0 {
			return Err("Not genesis".into());
		}
		self.block_interval.set(&params.block_interval)?;
		self.authority.set(&params.authority)?;
		Ok(())
	}

	fn validate_init(&self, _sender: Option<&Address>, params: InitParams) -> ModuleResult<()> {
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

	#[call(write = true)]
	fn update_authority(
		&self,
		sender: Option<&Address>,
		params: UpdateAuthorityParams,
	) -> ModuleResult<()> {
		let sender = sender.ok_or(ApplicationError::Unsigned)?;
		let current_authority = self.authority.get()?;
		let current_authority = current_authority.ok_or("Unexpected none")?;
		if sender != &current_authority {
			return Err("Not authority".into());
		}
		self.authority.set(&params.authority)?;
		self.context.emit_event(Event::from_data(
			"AuthorityUpdated".to_string(),
			AuthorityUpdated {
				authority: params.authority,
			},
		)?)?;

		Ok(())
	}

	fn validate_update_authority(
		&self,
		_sender: Option<&Address>,
		params: UpdateAuthorityParams,
	) -> ModuleResult<()> {
		self.util.validate_address(&params.authority)?;
		Ok(())
	}
}

#[derive(Encode, Decode, Debug, PartialEq, Deserialize)]
pub struct InitParams {
	pub block_interval: Option<u64>,
	pub authority: Address,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct UpdateAuthorityParams {
	pub authority: Address,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Meta {
	pub block_interval: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AuthorityUpdated {
	pub authority: Address,
}
