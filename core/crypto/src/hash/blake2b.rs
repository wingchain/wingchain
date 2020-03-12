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

use rust_crypto::blake2b;

use crate::hash::Hash;
use crate::KeyLength;

pub struct Blake2b256;

impl Hash for Blake2b256 {
	fn name(&self) -> &'static str {
		"blake2b256"
	}
	fn key_length(&self) -> KeyLength {
		KeyLength::KeyLength32
	}
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		blake2b::Blake2b::blake2b(out, data, &[]);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test() {
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		Blake2b256.hash(&mut out, &data);

		assert_eq!(
			out,
			[
				17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142,
				220, 157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
			]
		);
	}
}
