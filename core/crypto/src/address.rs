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

use std::path::PathBuf;
use std::str::FromStr;

use primitives::errors::CommonError;

use crate::address::blake2b::Blake2b160;
use crate::address::custom_lib::CustomLib;
use crate::address::original::{Original160, Original256};
use crate::AddressLength;

mod blake2b;
mod custom_lib;
mod original;

pub trait Address {
	fn name(&self) -> String;
	fn length(&self) -> AddressLength;
	fn address(&self, out: &mut [u8], data: &[u8]);
}

pub enum AddressImpl {
	/// blake2b 160 on public key
	Blake2b160,
	/// original 160 bits public key
	Original160,
	/// original 256 bits public key
	Original256,
	/// custom address impl provided by dylib
	Custom(CustomLib),
}

impl Address for AddressImpl {
	#[inline]
	fn name(&self) -> String {
		match self {
			Self::Blake2b160 => Blake2b160.name(),
			Self::Original160 => Original160.name(),
			Self::Original256 => Original256.name(),
			Self::Custom(custom) => custom.name(),
		}
	}
	#[inline]
	fn length(&self) -> AddressLength {
		match self {
			Self::Blake2b160 => Blake2b160.length(),
			Self::Original160 => Original160.length(),
			Self::Original256 => Original256.length(),
			Self::Custom(custom) => custom.length(),
		}
	}
	#[inline]
	fn address(&self, out: &mut [u8], data: &[u8]) {
		match self {
			Self::Blake2b160 => Blake2b160.address(out, data),
			Self::Original160 => Original160.address(out, data),
			Self::Original256 => Original256.address(out, data),
			Self::Custom(custom) => custom.address(out, data),
		}
	}
}

impl FromStr for AddressImpl {
	type Err = CommonError;
	#[inline]
	fn from_str(s: &str) -> Result<AddressImpl, Self::Err> {
		match s {
			"blake2b_160" => Ok(AddressImpl::Blake2b160),
			"original_160" => Ok(AddressImpl::Original160),
			"original_256" => Ok(AddressImpl::Original256),
			other => {
				let path = PathBuf::from(&other);
				let custom_lib = CustomLib::new(&path)?;
				Ok(AddressImpl::Custom(custom_lib))
			}
		}
	}
}
