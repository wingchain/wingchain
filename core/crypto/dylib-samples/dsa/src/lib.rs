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

use ed25519_dalek::{Keypair as DalekKeyPair, PublicKey, SecretKey, Signature};
use rand::rngs::OsRng;
use sha2::Sha512;

use crypto::dsa::{CLength, Dsa, KeyPair as KeyPairT, Verifier as VerifierT};
use crypto::DsaLength;

pub struct Ed25519;

#[derive(Debug)]
pub struct KeyPair(DalekKeyPair);

pub struct Verifier(PublicKey);

impl Dsa for Ed25519 {
	type Error = ();

	type KeyPair = KeyPair;

	type Verifier = Verifier;

	fn name(&self) -> String {
		"ed25519".to_string()
	}

	fn length(&self) -> DsaLength {
		DsaLength::DsaLength32_32_64
	}

	fn generate_key_pair(&self) -> Result<Self::KeyPair, Self::Error> {
		let mut csprng = OsRng::new().map_err(|_| ())?;
		let key_pair = KeyPair(DalekKeyPair::generate::<Sha512, _>(&mut csprng));
		Ok(key_pair)
	}

	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> Result<Self::KeyPair, Self::Error> {
		let secret_key = SecretKey::from_bytes(secret_key).map_err(|_| ())?;
		let public_key = PublicKey::from_secret::<Sha512>(&secret_key);
		let key_pair = KeyPair(DalekKeyPair {
			secret: secret_key,
			public: public_key,
		});
		Ok(key_pair)
	}

	fn verifier_from_public_key(&self, public_key: &[u8]) -> Result<Self::Verifier, Self::Error> {
		let public_key = PublicKey::from_bytes(public_key).map_err(|_| ())?;
		Ok(Verifier(public_key))
	}
}

impl KeyPairT for KeyPair {
	fn public_key(&self, out: &mut [u8]) {
		let public = self.0.public.as_bytes();
		out.copy_from_slice(public);
	}
	fn secret_key(&self, out: &mut [u8]) {
		let secret = self.0.secret.as_bytes();
		out.copy_from_slice(secret);
	}
	fn sign(&self, message: &[u8], out: &mut [u8]) {
		let signature = self.0.sign::<Sha512>(message).to_bytes();
		out.copy_from_slice(&signature);
	}
}

impl VerifierT for Verifier {
	type Error = ();

	fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Self::Error> {
		let signature = Signature::from_bytes(signature).map_err(|_| ())?;
		self.0.verify::<Sha512>(message, &signature).map_err(|_| ())
	}
}

declare_dsa_custom_lib!(Ed25519);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_ed25519_generate_key_pair() {
		assert!(Ed25519.generate_key_pair().is_ok());
	}

	#[test]
	fn test_ed25519_key_pair_from_secret_key() {
		let secret: [u8; 32] = [
			184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29,
			148, 3, 77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
		];
		assert!(Ed25519.key_pair_from_secret_key(&secret).is_ok());
	}

	#[test]
	fn test_ed25519_key_pair() {
		let secret: [u8; 32] = [
			184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29,
			148, 3, 77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
		];

		let key_pair = Ed25519.key_pair_from_secret_key(&secret).unwrap();

		let mut public_key = [0u8; 32];
		key_pair.public_key(&mut public_key);

		assert_eq!(
			public_key.to_vec(),
			vec![
				137, 44, 137, 164, 205, 99, 29, 8, 218, 49, 70, 7, 34, 56, 20, 119, 86, 4, 83, 90,
				5, 245, 14, 149, 157, 33, 32, 157, 1, 116, 14, 186
			]
		);

		let message: Vec<u8> = vec![97, 98, 99];

		let mut signature = [0u8; 64];
		key_pair.sign(&message, &mut signature);

		assert_eq!(
			signature.to_vec(),
			vec![
				82, 19, 26, 105, 235, 178, 54, 112, 61, 224, 195, 88, 150, 137, 32, 46, 235, 209,
				209, 108, 64, 153, 12, 58, 216, 179, 88, 38, 49, 167, 162, 103, 219, 116, 93, 187,
				145, 86, 216, 98, 97, 135, 228, 15, 66, 246, 207, 232, 132, 182, 211, 206, 12, 220,
				4, 96, 58, 254, 237, 8, 151, 3, 172, 14
			]
		);

		let verifier = Ed25519.verifier_from_public_key(&public_key).unwrap();

		let result = verifier.verify(&message, &signature);

		assert!(result.is_ok());
	}
}
