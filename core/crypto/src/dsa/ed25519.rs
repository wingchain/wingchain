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

use crate::dsa::Dsa;
use crate::errors;
use rand::thread_rng;
use rand::Rng;
use ring::signature::Ed25519KeyPair;

pub struct Ed25519;

impl Dsa for Ed25519 {
	type KeyPair = Ed25519KeyPair;

	fn generate_key_pair(&self) -> errors::Result<Self::KeyPair> {
		let seed = random_32_bytes(&mut thread_rng());

		let key_pair = Ed25519KeyPair::from_seed_unchecked(&seed)?;

		Ok(key_pair)
	}

	fn key_pair_from_secret(&self, secret: &[u8]) -> errors::Result<Self::KeyPair> {
		let key_pair = Ed25519KeyPair::from_seed_unchecked(&secret)?;

		Ok(key_pair)
	}
}

fn random_32_bytes<R: Rng + ?Sized>(rng: &mut R) -> [u8; 32] {
	let mut ret = [0u8; 32];
	rng.fill_bytes(&mut ret);
	ret
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_ed25519_generate_key_pair() {
		assert!(Ed25519.generate_key_pair().is_ok());
	}

	#[test]
	fn test_ed25519_key_pair_from_secret() {
		let secret : [u8; 32] = [
			184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29,
			148, 3, 77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
		];
		assert!(Ed25519.key_pair_from_secret(&secret).is_ok());
	}
}
