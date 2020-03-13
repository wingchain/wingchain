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

#[macro_use]
extern crate crypto;

use rust_crypto::digest::Digest;
use rust_crypto::sha1;

use crypto::hash::Hash;
use crypto::KeyLength;

pub struct SampleHash;

/// An SHA-1 implementation for sample
impl Hash for SampleHash {
	fn name(&self) -> &'static str {
		"sample"
	}
	fn key_length(&self) -> KeyLength {
		KeyLength::KeyLength20
	}
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		let mut hasher = sha1::Sha1::new();
		hasher.input(data);
		hasher.result(out);
	}
}

declare_custom_lib!(SampleHash);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test() {
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 20];
		SampleHash.hash(&mut out, &data);

		assert_eq!(
			out,
			[
				112, 55, 128, 113, 152, 194, 42, 125, 43, 8, 7, 55, 29, 118, 55, 121, 168, 79, 223,
				207
			]
		);
	}
}
