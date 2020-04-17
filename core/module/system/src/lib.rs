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

use serde::{Deserialize, Serialize};

use node_executor_primitives::{errors, Context, Module as ModuleT, StorageValue};
use primitives::errors::CommonResult;
use primitives::{codec, Call};

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

	fn is_valid_call(call: &Call) -> bool {
		let params = &call.params.0[..];
		match call.method.as_str() {
			"init" => codec::decode::<InitParams>(&params).is_ok(),
			_ => false,
		}
	}

	fn is_write_call(call: &Call) -> Option<bool> {
		let write_methods = vec!["init"];
		Some(write_methods.contains(&call.method.as_str()))
	}

	fn execute_call(&self, call: &Call) -> CommonResult<()> {
		let params = &call.params.0[..];
		match call.method.as_str() {
			"init" => {
				self.init(&codec::decode(&params).map_err(|_| errors::ErrorKind::InvalidParams)?)
			}
			other => Err(errors::ErrorKind::InvalidMethod(other.to_string()).into()),
		}
	}
}

#[derive(Serialize, Deserialize)]
pub struct InitParams {
	pub chain_id: String,
	pub timestamp: u32,
}

impl<C: Context> Module<C> {
	fn init(&self, params: &InitParams) -> CommonResult<()> {
		self.chain_id.set(&params.chain_id)?;
		self.timestamp.set(&params.timestamp)?;
		Ok(())
	}
}
