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

use yogcrypt::sm3::sm3_enc;

use crate::hash::Hash;
use crate::KeyLength;

pub struct SM3;

impl Hash for SM3 {
	fn name(&self) -> String {
		"sm3".to_string()
	}
	fn key_length(&self) -> KeyLength {
		KeyLength::KeyLength32
	}
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		let result = sm3_enc(data);

		for i in 0..8 {
			let bytes = result[i].to_be_bytes();
			out[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test() {
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		SM3.hash(&mut out, &data);

		assert_eq!(
			out,
			[
				158, 139, 109, 83, 238, 96, 25, 26, 219, 93, 71, 130, 155, 7, 70, 50, 56, 171, 15,
				159, 227, 157, 222, 97, 216, 238, 73, 54, 50, 158, 49, 251
			]
		);
	}
}
