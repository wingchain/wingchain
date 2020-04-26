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

use executor_macro::{call, module};
use executor_primitives::{errors, Context, Module as ModuleT, StorageMap};
use primitives::errors::CommonResult;
use primitives::{codec, Address, Balance, Call};

pub struct Module<C>
where
	C: Context,
{
	balance: StorageMap<Address, Balance, C>,
}

#[module]
impl<C: Context> Module<C> {
	const META_MODULE: bool = false;
	const STORAGE_KEY: &'static [u8] = b"balance";

	fn new(context: C) -> Self {
		Self {
			balance: StorageMap::new::<Self>(context, b"blanace"),
		}
	}

	#[call]
	fn get_balance(&self, params: EmptyParams) -> CommonResult<()> {
		Ok(())
	}

	#[call(write = true)]
	fn transfer(&self, params: EmptyParams) -> CommonResult<()> {
		Ok(())
	}
}

#[derive(Deserialize)]
struct EmptyParams;
