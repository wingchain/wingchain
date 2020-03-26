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

use std::str::FromStr;

use crypto::dsa::Dsa;
use crypto::dsa::DsaImpl;
use crypto::dsa::KeyPair;
use crypto::dsa::Verifier;

#[test]
fn test_from_ed25519() {
	let dsa_impl = DsaImpl::from_str("ed25519").unwrap();

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let (_secret_len, public_len, sig_len) = dsa_impl.length().into();

	let key_pair = dsa_impl.key_pair_from_secret_key(&secret).unwrap();

	let mut public_key = vec![0u8; public_len];
	key_pair.public_key(&mut public_key);

	assert_eq!(
		public_key,
		vec![
			137, 44, 137, 164, 205, 99, 29, 8, 218, 49, 70, 7, 34, 56, 20, 119, 86, 4, 83, 90, 5,
			245, 14, 149, 157, 33, 32, 157, 1, 116, 14, 186
		]
	);

	let message: Vec<u8> = vec![97, 98, 99];

	let mut signature = vec![0u8; sig_len];
	key_pair.sign(&message, &mut signature);

	assert_eq!(
		signature,
		vec![
			82, 19, 26, 105, 235, 178, 54, 112, 61, 224, 195, 88, 150, 137, 32, 46, 235, 209, 209,
			108, 64, 153, 12, 58, 216, 179, 88, 38, 49, 167, 162, 103, 219, 116, 93, 187, 145, 86,
			216, 98, 97, 135, 228, 15, 66, 246, 207, 232, 132, 182, 211, 206, 12, 220, 4, 96, 58,
			254, 237, 8, 151, 3, 172, 14
		]
	);

	let verifier = dsa_impl.verifier_from_public_key(&public_key).unwrap();

	let result = verifier.verify(&message, &signature);

	assert!(result.is_ok());
}

#[test]
fn test_from_ed25519_generate_key_pair() {
	let dsa_impl = DsaImpl::from_str("ed25519").unwrap();
	let key_pair = dsa_impl.generate_key_pair();
	assert!(key_pair.is_ok());
}

#[test]
fn test_from_sm2() {
	let dsa_impl = DsaImpl::from_str("sm2").unwrap();

	let secret: [u8; 32] = [
		128, 166, 19, 115, 227, 79, 114, 21, 254, 206, 184, 221, 6, 187, 55, 49, 234, 54, 47, 245,
		53, 90, 114, 38, 212, 225, 45, 7, 106, 126, 181, 136,
	];
	let key_pair = dsa_impl.key_pair_from_secret_key(&secret).unwrap();

	let (_secret_len, public_len, sig_len) = dsa_impl.length().into();

	let mut public_key = vec![0u8; public_len];

	key_pair.public_key(&mut public_key);

	assert_eq!(
		public_key,
		vec![
			4, 75, 45, 216, 191, 109, 187, 251, 20, 219, 62, 77, 23, 189, 122, 62, 135, 88, 235,
			66, 50, 4, 155, 236, 147, 29, 16, 56, 244, 175, 170, 228, 106, 195, 199, 113, 249, 41,
			187, 243, 90, 40, 176, 54, 55, 137, 251, 25, 18, 124, 234, 51, 24, 244, 200, 144, 42,
			0, 52, 202, 95, 27, 118, 103, 209
		]
	);

	let message: Vec<u8> = vec![97, 98, 99];

	let mut signature = vec![0u8; sig_len];
	key_pair.sign(&message, &mut signature);

	assert_eq!(signature.len(), 64);

	let verifier = dsa_impl.verifier_from_public_key(&public_key).unwrap();

	let result = verifier.verify(&message, &signature);

	assert!(result.is_ok());
}

#[test]
fn test_from_sm2_generate_key_pair() {
	let dsa_impl = DsaImpl::from_str("sm2").unwrap();
	let key_pair = dsa_impl.generate_key_pair();
	assert!(key_pair.is_ok());
}
