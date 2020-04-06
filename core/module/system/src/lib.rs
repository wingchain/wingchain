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

use hash_enum::HashEnum;
use node_executor_primitives::{
	errors::ErrorKind, errors::Result, Context, Module as ModuleT, StorageValue,
};
use parity_codec::{Decode, Encode};
use primitives::{Call, FromDispatchId};

#[derive(HashEnum)]
pub enum MethodEnum {
	Init,
}

pub struct Module<C>
where
	C: Context,
{
	chain_id: StorageValue<String, C>,
	timestamp: StorageValue<u32, C>,
}

impl<C> ModuleT<C> for Module<C>
where
	C: Context,
{
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"system";

	fn new(context: C) -> Self {
		Self {
			chain_id: StorageValue::new::<Self>(context.clone(), b"chain_id"),
			timestamp: StorageValue::new::<Self>(context, b"timestamp"),
		}
	}

	fn validate_call(call: &Call) -> bool {
		let call_enum = match MethodEnum::from_dispatch_id(&call.method_id) {
			Some(call_enum) => call_enum,
			None => return false,
		};
		let mut params = &call.params.0[..];
		match call_enum {
			MethodEnum::Init => match <InitParams as Decode>::decode(&mut params) {
				Ok(_) => true,
				Err(_) => false,
			},
		}
	}

	fn execute_call(&self, call: &Call) -> Result<()> {
		let call_enum = match MethodEnum::from_dispatch_id(&call.method_id) {
			Some(call_enum) => call_enum,
			None => return Err(ErrorKind::InvalidDispatchId(call.module_id.clone()).into()),
		};
		let mut params = &call.params.0[..];
		match call_enum {
			MethodEnum::Init => {
				let params = Decode::decode(&mut params).map_err(|_| ErrorKind::InvalidParams)?;
				self.init(&params)
			}
		}
	}
}

#[derive(Encode, Decode)]
pub struct InitParams {
	pub chain_id: String,
	pub timestamp: u32,
}

impl<C: Context> Module<C> {
	fn init(&self, params: &InitParams) -> Result<()> {
		self.chain_id.set(&params.chain_id)?;
		self.timestamp.set(&params.timestamp)?;
		Ok(())
	}
}
