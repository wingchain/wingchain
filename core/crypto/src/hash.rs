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

use crate::errors;
use crate::KeyLength;
use blake2b::Blake2b256;
use sm3::SM3;
use std::str::FromStr;

pub mod blake2b;
pub mod sm3;

pub trait Hash {
	fn name(&self) -> &'static str;
	fn key_length(&self) -> KeyLength;
	fn hash(&self, out: &mut [u8], data: &[u8]);
}

pub enum HashImpl {
	Blake2b256,
	SM3,
}

impl Hash for HashImpl {
	#[inline]
	fn name(&self) -> &'static str {
		match self {
			Self::Blake2b256 => Blake2b256.name(),
			Self::SM3 => SM3.name(),
		}
	}
	#[inline]
	fn key_length(&self) -> KeyLength {
		match self {
			Self::Blake2b256 => Blake2b256.key_length(),
			Self::SM3 => SM3.key_length(),
		}
	}
	#[inline]
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		match self {
			Self::Blake2b256 => Blake2b256.hash(out, data),
			Self::SM3 => SM3.hash(out, data),
		}
	}
}

impl FromStr for HashImpl {
	type Err = errors::Error;
	#[inline]
	fn from_str(s: &str) -> Result<HashImpl, Self::Err> {
		match s {
			"blake2b256" => Ok(HashImpl::Blake2b256),
			"sm3" => Ok(HashImpl::SM3),
			_ => Err(errors::ErrorKind::HashNameNotFound.into()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test() {
		let hash = HashImpl::Blake2b256;
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		hash.hash(&mut out, &data);
		assert_eq!(
			out,
			[
				17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142,
				220, 157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
			]
		);
	}

	#[test]
	fn test_from_str() {
		let hash = HashImpl::from_str("sm3").unwrap();
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		hash.hash(&mut out, &data);
		assert_eq!(
			out,
			[
				158, 139, 109, 83, 238, 96, 25, 26, 219, 93, 71, 130, 155, 7, 70, 50, 56, 171, 15,
				159, 227, 157, 222, 97, 216, 238, 73, 54, 50, 158, 49, 251
			]
		);
	}
}
