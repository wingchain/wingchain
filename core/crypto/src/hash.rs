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

use blake2b::{Blake2b160, Blake2b256, Blake2b512};
use sm3::SM3;

use crate::errors;
use crate::hash::custom_lib::CustomLib;
use crate::HashLength;

mod blake2b;
mod custom_lib;
mod sm3;

pub trait Hash {
	fn name(&self) -> String;
	fn key_length(&self) -> HashLength;
	fn hash(&self, out: &mut [u8], data: &[u8]);
}

pub enum HashImpl {
	Blake2b160,
	Blake2b256,
	Blake2b512,
	SM3,
	/// custom hash impl provided by dylib
	Custom(CustomLib),
}

impl Hash for HashImpl {
	#[inline]
	fn name(&self) -> String {
		match self {
			Self::Blake2b160 => Blake2b160.name(),
			Self::Blake2b256 => Blake2b256.name(),
			Self::Blake2b512 => Blake2b512.name(),
			Self::SM3 => SM3.name(),
			Self::Custom(custom) => custom.name(),
		}
	}
	#[inline]
	fn key_length(&self) -> HashLength {
		match self {
			Self::Blake2b160 => Blake2b160.key_length(),
			Self::Blake2b256 => Blake2b256.key_length(),
			Self::Blake2b512 => Blake2b512.key_length(),
			Self::SM3 => SM3.key_length(),
			Self::Custom(custom) => custom.key_length(),
		}
	}
	#[inline]
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		match self {
			Self::Blake2b160 => Blake2b160.hash(out, data),
			Self::Blake2b256 => Blake2b256.hash(out, data),
			Self::Blake2b512 => Blake2b512.hash(out, data),
			Self::SM3 => SM3.hash(out, data),
			Self::Custom(custom) => custom.hash(out, data),
		}
	}
}

impl FromStr for HashImpl {
	type Err = errors::Error;
	#[inline]
	fn from_str(s: &str) -> Result<HashImpl, Self::Err> {
		match s {
			"blake2b_160" => Ok(HashImpl::Blake2b160),
			"blake2b_256" => Ok(HashImpl::Blake2b256),
			"blake2b_512" => Ok(HashImpl::Blake2b512),
			"sm3" => Ok(HashImpl::SM3),
			other => {
				let path = PathBuf::from(&other);
				let custom_lib = CustomLib::new(&path)?;
				Ok(HashImpl::Custom(custom_lib))
			}
		}
	}
}
