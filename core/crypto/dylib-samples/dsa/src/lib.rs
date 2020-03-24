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

use ed25519_dalek::{Keypair as DalekKeyPair, PublicKey, SecretKey, Signature};
use rand::rngs::OsRng;
use sha2::Sha512;

use crypto::dsa::{CDsa, CKeyPair as KeyPairT, CVerifier as VerifierT};

pub struct Ed25519;

#[derive(Debug)]
pub struct KeyPair(DalekKeyPair);

pub struct Verifier(PublicKey);

impl CDsa for Ed25519 {
	const ERR_LEN: usize = 18;

	type KeyPair = KeyPair;

	type Verifier = Verifier;

	fn name(&self) -> String {
		"ed25519".to_string()
	}

	fn generate_key_pair(&self) -> Result<Self::KeyPair, Vec<u8>> {
		let mut csprng = OsRng::new().map_err(|_| b"invalid secret key".to_vec())?;
		let key_pair = KeyPair(DalekKeyPair::generate::<Sha512, _>(&mut csprng));
		Ok(key_pair)
	}

	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> Result<Self::KeyPair, Vec<u8>> {
		let secret_key =
			SecretKey::from_bytes(secret_key).map_err(|_| b"invalid secret key".to_vec())?;
		let public_key = PublicKey::from_secret::<Sha512>(&secret_key);
		let key_pair = KeyPair(DalekKeyPair {
			secret: secret_key,
			public: public_key,
		});
		Ok(key_pair)
	}

	fn verifier_from_public_key(&self, public_key: &[u8]) -> Result<Self::Verifier, Vec<u8>> {
		let public_key =
			PublicKey::from_bytes(public_key).map_err(|_| b"invalid secret key".to_vec())?;
		Ok(Verifier(public_key))
	}
}

impl KeyPairT for KeyPair {
	const PUBLIC_LEN: usize = 32;
	const SECRET_LEN: usize = 32;
	const SIGNATURE_LEN: usize = 64;
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
	const ERR_LEN: usize = 19;
	fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Vec<u8>> {
		let signature =
			Signature::from_bytes(signature).map_err(|_| b"verification failed".to_vec())?;
		self.0
			.verify::<Sha512>(message, &signature)
			.map_err(|_| b"verification failed".to_vec())
	}
}

use std::os::raw::{c_uchar, c_uint, c_char};
use std::ptr::null_mut;
use std::slice;
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_name() -> *mut c_char {
	let name = Ed25519.name();
	CString::new(name).expect("qed").into_raw()
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_name_free(name: *mut c_char) {
	unsafe {
		assert!(!name.is_null());
		CString::from_raw(name)
	};
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_generate_key_pair(
	err: *mut c_uchar,
	err_len: *mut c_uint,
) -> *mut KeyPair {
	let key_pair = match Ed25519.generate_key_pair() {
		Ok(v) => v,
		Err(e) => {
			crypto_dsa_error_handle(e, err, err_len);
			return null_mut() as *mut _;
		}
	};
	Box::into_raw(Box::new(key_pair))
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_key_pair_from_secret_key(
	secret_key: *const c_uchar,
	secret_key_len: c_uint,
	err: *mut c_uchar,
	err_len: *mut c_uint,
) -> *mut KeyPair {
	let secret_key = unsafe { slice::from_raw_parts(secret_key, secret_key_len as usize) };
	let key_pair = match Ed25519.key_pair_from_secret_key(secret_key) {
		Ok(v) => v,
		Err(e) => {
			crypto_dsa_error_handle(e, err, err_len);
			return null_mut() as *mut _;
		}
	};
	Box::into_raw(Box::new(key_pair))
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_key_pair_secret_key(
	key_pair: *mut KeyPair,
	out: *mut c_uchar,
	out_len: c_uint,
) {
	let key_pair = unsafe { Box::from_raw(key_pair) };
	let mut out = unsafe { slice::from_raw_parts_mut(out, out_len as usize) };
	key_pair.secret_key(&mut out);
	std::mem::forget(key_pair);
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_key_pair_public_key(
	key_pair: *mut KeyPair,
	out: *mut c_uchar,
	out_len: c_uint,
) {
	let key_pair = unsafe { Box::from_raw(key_pair) };
	let mut out = unsafe { slice::from_raw_parts_mut(out, out_len as usize) };
	key_pair.public_key(&mut out);
	std::mem::forget(key_pair);
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_key_pair_sign(
	key_pair: *mut KeyPair,
	message: *const c_uchar,
	message_len: c_uint,
	out: *mut c_uchar,
	out_len: c_uint,
) {
	let key_pair = unsafe { Box::from_raw(key_pair) };
	let message = unsafe { slice::from_raw_parts(message, message_len as usize) };
	let mut out = unsafe { slice::from_raw_parts_mut(out, out_len as usize) };
	key_pair.sign(message, &mut out);
	std::mem::forget(key_pair);
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_key_pair_free(
	key_pair: *mut KeyPair,
) {
	unsafe { Box::from_raw(key_pair) };
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_verifier_from_public_key(
	public_key: *const c_uchar,
	public_key_len: c_uint,
	err: *mut c_uchar,
	err_len: *mut c_uint,
) -> *mut Verifier {
	let public_key = unsafe { slice::from_raw_parts(public_key, public_key_len as usize) };
	let verifier = match Ed25519.verifier_from_public_key(public_key) {
		Ok(v) => v,
		Err(e) => {
			crypto_dsa_error_handle(e, err, err_len);
			return null_mut() as *mut _;
		}
	};
	Box::into_raw(Box::new(verifier))
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_verifier_verify(
	verifier: *mut Verifier,
	message: *const c_uchar,
	message_len: c_uint,
	signature: *const c_uchar,
	signature_len: c_uint,
	err: *mut c_uchar,
	err_len: *mut c_uint,
) {
	let verifier = unsafe { Box::from_raw(verifier) };

	let message = unsafe { slice::from_raw_parts(message, message_len as usize) };
	let signature = unsafe { slice::from_raw_parts(signature, signature_len as usize) };

	match verifier.verify(message, signature) {
		Ok(_) => (),
		Err(e) => {
			//println!("{:?}", String::from_utf8(e).unwrap());
			crypto_dsa_error_handle(e, err, err_len);
		}
	}
	std::mem::forget(verifier);
}

#[no_mangle]
pub extern "C" fn _crypto_dsa_custom_verifier_free(
	verifier: *mut Verifier,
) {
	unsafe { Box::from_raw(verifier) };
}

fn crypto_dsa_error_handle(e: Vec<u8>, err: *mut c_uchar, err_len: *mut c_uint) {
	let len = e.len();
	let err = unsafe { slice::from_raw_parts_mut(err, len) };
	err.copy_from_slice(&e);
	unsafe {
		*err_len = len as c_uint;
	}
}

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
