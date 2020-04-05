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
use parity_codec::{Decode, Encode};
use primitives::traits::Module as ModuleT;
use primitives::{Call, DispatchId, FromDispatchId, Params};
use node_executor_primitives::{StorageValue, Context};

#[derive(HashEnum)]
pub enum MethodEnum {
	Init,
}

#[derive(Encode, Decode)]
pub struct InitParams {
	pub chain_id: String,
	pub timestamp: u32,
}

impl MethodEnum {
	fn validate_call(&self, call: &Call) -> bool {
		let mut params = &call.params.0[..];
		match *self {
			MethodEnum::Init => {
				match <InitParams as Decode>::decode(&mut params) {
					Some(_params) => (),
					None => return false,
				};
			}
		}
		true
	}
}

pub struct Module<C>
	where C: Context
{
	chain_id: StorageValue<String, C>,
	time: StorageValue<u32, C>,
}

impl<C> ModuleT for Module<C>
	where C: Context
{
	const META_MODULE: bool = true;
	const STORAGE_KEY: &'static [u8] = b"system";

	fn validate_call(call: &Call) -> bool {
		let call_enum = match MethodEnum::from_dispatch_id(&call.method_id) {
			Some(call_enum) => call_enum,
			None => return false,
		};
		call_enum.validate_call(call)
	}
}

impl<C> Module<C>
	where C: Context{
	fn new(context: C) -> Self{
		Self{
			chain_id: StorageValue::new::<Self>(context.clone(), b"chain_id"),
			time: StorageValue::new::<Self>(context, b"time"),
		}
	}
}
