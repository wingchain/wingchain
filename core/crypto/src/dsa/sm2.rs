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

use std::iter::{empty, once};

use yogcrypt::basic::cell::u64x4::U64x4;
use yogcrypt::basic::field::field_p::FieldElement;
use yogcrypt::sm2::{
	get_pub_key, get_sec_key, sm2_gen_sign, sm2_ver_sign, PubKey, SecKey, Signature,
};

use primitives::errors::{CommonError, CommonResult};

use crate::dsa::{Dsa, KeyPair, Verifier};
use crate::{errors, DsaLength};

pub struct SM2;

impl Dsa for SM2 {
	type Error = CommonError;

	type KeyPair = (SecKey, PubKey);
	type Verifier = PubKey;

	fn name(&self) -> String {
		"sm2".to_string()
	}

	fn length(&self) -> DsaLength {
		DsaLength::DsaLength32_65_64
	}

	fn generate_key_pair(&self) -> CommonResult<Self::KeyPair> {
		let secret_key = get_sec_key();
		let public_key = get_pub_key(secret_key);
		Ok((secret_key, public_key))
	}

	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> CommonResult<Self::KeyPair> {
		let secret_key = slice_to_secret_key(secret_key)?;
		let public_key = get_pub_key(secret_key);
		Ok((secret_key, public_key))
	}

	fn verifier_from_public_key(&self, public_key: &[u8]) -> CommonResult<Self::Verifier> {
		let public_key = slice_to_public_key(public_key)?;
		Ok(public_key)
	}
}

impl KeyPair for (SecKey, PubKey) {
	fn public_key(&self, out: &mut [u8]) {
		out.copy_from_slice(&public_key_to_vec(self.1));
	}
	fn secret_key(&self, out: &mut [u8]) {
		out.copy_from_slice(&secret_key_to_vec(self.0));
	}
	fn sign(&self, message: &[u8], out: &mut [u8]) {
		let signature = sm2_gen_sign(&message, self.0, self.1);

		let signature = signature_to_vec(signature);
		out.copy_from_slice(&signature);
	}
}

impl Verifier for PubKey {
	type Error = CommonError;

	fn verify(&self, message: &[u8], signature: &[u8]) -> CommonResult<()> {
		let signature = slice_to_signature(signature)?;

		let verified = sm2_ver_sign(message, self.to_owned(), &signature);

		match verified {
			true => Ok(()),
			false => Err(errors::ErrorKind::VerificationFailed.into()),
		}
	}
}

fn slice_to_secret_key(slice: &[u8]) -> CommonResult<SecKey> {
	if slice.len() != 32 {
		return Err(errors::ErrorKind::InvalidSecretKey.into());
	}

	let secret_key = slice_to_u64x4(&slice);

	Ok(secret_key)
}

fn slice_to_public_key(slice: &[u8]) -> CommonResult<PubKey> {
	if slice.len() != 65 || slice[0] != 4 {
		return Err(errors::ErrorKind::InvalidPublicKey.into());
	}

	let x_slice = &slice[1..33];
	let y_slice = &slice[33..65];
	let public_key = PubKey {
		x: FieldElement {
			num: slice_to_u64x4(x_slice),
		},
		y: FieldElement {
			num: slice_to_u64x4(y_slice),
		},
	};

	Ok(public_key)
}

fn slice_to_u64x4(slice: &[u8]) -> U64x4 {
	U64x4 {
		value: [
			u64::from_be_bytes({ slice_to_arr(&slice[24..32]) }),
			u64::from_be_bytes(slice_to_arr(&slice[16..24])),
			u64::from_be_bytes(slice_to_arr(&slice[8..16])),
			u64::from_be_bytes(slice_to_arr(&slice[0..8])),
		],
	}
}

fn slice_to_arr(slice: &[u8]) -> [u8; 8] {
	let mut a = [0u8; 8];
	a.copy_from_slice(slice);
	a
}

// Referece: http://www.jonllen.com/upload/jonllen/case/jsrsasign-master/sample-sm2_crypt.html
fn secret_key_to_vec(secret_key: SecKey) -> Vec<u8> {
	let result: Vec<u8> = secret_key
		.value
		.iter()
		.rev()
		.map(|x| x.to_be_bytes().to_vec())
		.flatten()
		.collect();
	result
}

fn public_key_to_vec(public_key: PubKey) -> Vec<u8> {
	let result = once(4u8) // uncompressed
		.chain(
			public_key
				.x
				.num
				.value
				.iter()
				.rev()
				.map(|i| i.to_be_bytes().to_vec())
				.flatten(),
		)
		.chain(
			public_key
				.y
				.num
				.value
				.iter()
				.rev()
				.map(|i| i.to_be_bytes().to_vec())
				.flatten(),
		)
		.collect::<Vec<_>>();
	result
}

fn signature_to_vec(sig: Signature) -> Vec<u8> {
	let result = empty()
		.chain(
			sig.r
				.value
				.iter()
				.rev()
				.map(|i| i.to_be_bytes().to_vec())
				.flatten(),
		)
		.chain(
			sig.s
				.value
				.iter()
				.rev()
				.map(|i| i.to_be_bytes().to_vec())
				.flatten(),
		)
		.collect::<Vec<_>>();
	result
}

fn slice_to_signature(slice: &[u8]) -> CommonResult<Signature> {
	if slice.len() != 64 {
		return Err(errors::ErrorKind::VerificationFailed.into());
	}

	let r_slice = &slice[0..32];
	let s_slice = &slice[32..64];
	let signature = Signature {
		r: slice_to_u64x4(r_slice),
		s: slice_to_u64x4(s_slice),
	};

	Ok(signature)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sm2_generate_key_pair() {
		assert!(SM2.generate_key_pair().is_ok());
	}

	#[test]
	fn test_sm2_key_pair() {
		let secret: [u8; 32] = [
			128, 166, 19, 115, 227, 79, 114, 21, 254, 206, 184, 221, 6, 187, 55, 49, 234, 54, 47,
			245, 53, 90, 114, 38, 212, 225, 45, 7, 106, 126, 181, 136,
		];
		let key_pair = SM2.key_pair_from_secret_key(&secret).unwrap();

		let (_secret_len, public_len, sig_len) = SM2.length().into();

		let mut public_key = vec![0u8; public_len];
		key_pair.public_key(&mut public_key);

		assert_eq!(
			public_key,
			vec![
				4, 75, 45, 216, 191, 109, 187, 251, 20, 219, 62, 77, 23, 189, 122, 62, 135, 88,
				235, 66, 50, 4, 155, 236, 147, 29, 16, 56, 244, 175, 170, 228, 106, 195, 199, 113,
				249, 41, 187, 243, 90, 40, 176, 54, 55, 137, 251, 25, 18, 124, 234, 51, 24, 244,
				200, 144, 42, 0, 52, 202, 95, 27, 118, 103, 209
			]
		);

		let message: Vec<u8> = vec![97, 98, 99];

		let mut signature = vec![0u8; sig_len];
		key_pair.sign(&message, &mut signature);

		assert_eq!(signature.len(), 64);

		let verifier = SM2.verifier_from_public_key(&public_key).unwrap();

		let result = verifier.verify(&message, &signature);

		assert!(result.is_ok());
	}
}
