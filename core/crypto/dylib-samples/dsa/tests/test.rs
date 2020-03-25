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

#![cfg(feature = "build-dep-test")]

use std::os::raw::{c_char, c_uchar, c_uint};

use crypto::dsa::{CDsaConf, Dsa, DsaImpl, KeyPair, Verifier};
use libloading::{Library, Symbol};
use std::ffi::CStr;
use std::str::FromStr;

#[test]
fn test_custom_lib_dsa() {
	let path = utils::get_dylib("crypto_dylib_samples_dsa");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let dsa = DsaImpl::from_str(&path).unwrap();

	assert_eq!("ed25519", dsa.name());

	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];

	let key_pair = dsa.key_pair_from_secret_key(&secret).unwrap();

	// secret
	let dump_secret = key_pair.secret_key();

	assert_eq!(dump_secret, secret);

	// public
	let public_key = key_pair.public_key();

	assert_eq!(
		public_key,
		vec![
			137, 44, 137, 164, 205, 99, 29, 8, 218, 49, 70, 7, 34, 56, 20, 119, 86, 4, 83, 90, 5,
			245, 14, 149, 157, 33, 32, 157, 1, 116, 14, 186
		]
	);

	// sign
	let message = [97u8, 98, 99];
	let signature = key_pair.sign(&message);

	assert_eq!(
		signature,
		vec![
			82, 19, 26, 105, 235, 178, 54, 112, 61, 224, 195, 88, 150, 137, 32, 46, 235, 209, 209,
			108, 64, 153, 12, 58, 216, 179, 88, 38, 49, 167, 162, 103, 219, 116, 93, 187, 145, 86,
			216, 98, 97, 135, 228, 15, 66, 246, 207, 232, 132, 182, 211, 206, 12, 220, 4, 96, 58,
			254, 237, 8, 151, 3, 172, 14
		]
	);

	// verify
	let verifier = dsa.verifier_from_public_key(&public_key).unwrap();

	assert!(verifier.verify(&message, &signature).is_ok());
}

#[test]
fn test_dylib_dsa() {
	let path = utils::get_dylib("crypto_dylib_samples_dsa");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	#[repr(C)]
	pub struct TKeyPair {
		_unused: [u8; 0],
	}

	#[repr(C)]
	pub struct TVerifier {
		_unused: [u8; 0],
	}

	let lib = Library::new(path).unwrap();

	type CallName = unsafe extern "C" fn() -> *mut c_char;
	type CallNameFree = unsafe extern "C" fn(*mut c_char);

	type CallGenerateKeyPair =
		unsafe extern "C" fn(err: *mut c_uchar, err_len: *mut c_uint) -> *mut TKeyPair;
	type CallKeyPairFromSecretKey = unsafe extern "C" fn(
		secret_key: *const c_uchar,
		secret_key_len: c_uint,
		err: *mut c_uchar,
		err_len: *mut c_uint,
	) -> *mut TKeyPair;
	type CallKeyPairSecretKey =
		unsafe extern "C" fn(key_pair: *mut TKeyPair, out: *mut c_uchar, out_len: c_uint);
	type CallKeyPairPublicKey =
		unsafe extern "C" fn(key_pair: *mut TKeyPair, out: *mut c_uchar, out_len: c_uint);
	type CallKeyPairSign = unsafe extern "C" fn(
		key_pair: *mut TKeyPair,
		message: *const c_uchar,
		message_len: c_uint,
		out: *mut c_uchar,
		out_len: c_uint,
	);
	type CallKeyPairFree = unsafe extern "C" fn(key_pair: *mut TKeyPair);
	type CallVerifierFromPublicKey = unsafe extern "C" fn(
		public_key: *const c_uchar,
		public_key_len: c_uint,
		err: *mut c_uchar,
		err_len: *mut c_uint,
	) -> *mut TVerifier;
	type CallVerifierVerify = unsafe extern "C" fn(
		verifier: *mut TVerifier,
		message: *const c_uchar,
		message_len: c_uint,
		signature: *const c_uchar,
		signature_len: c_uint,
		err: *mut c_uchar,
		err_len: *mut c_uint,
	);
	type CallVerifierFree = unsafe extern "C" fn(verifier: *mut TVerifier);

	type CallConf = unsafe extern "C" fn() -> CDsaConf;

	// conf
	let conf: CDsaConf = unsafe {
		let call_conf: Symbol<CallConf> = lib.get(b"_crypto_dsa_custom_conf").unwrap();
		let conf = call_conf();
		conf
	};
	assert_eq!(18, conf.err_len);

	// name
	let name: String = unsafe {
		let call_name: Symbol<CallName> = lib.get(b"_crypto_dsa_custom_name").unwrap();
		let call_name_free: Symbol<CallNameFree> =
			lib.get(b"_crypto_dsa_custom_name_free").unwrap();
		let raw = call_name();
		let name = CStr::from_ptr(raw).to_str().unwrap();
		let name = name.to_owned();
		call_name_free(raw);
		name
	};
	assert_eq!(name, "ed25519".to_string());

	// generate key pair
	unsafe {
		let call_generate_key_pair: Symbol<CallGenerateKeyPair> =
			lib.get(b"_crypto_dsa_custom_generate_key_pair").unwrap();
		let (mut err, mut err_len) = (vec![0u8; conf.err_len as usize], 0u32 as c_uint);
		let raw = call_generate_key_pair(err.as_mut_ptr(), &mut err_len as *mut c_uint);
		match err_len {
			0 => Ok(raw),
			_ => Err(String::from_utf8(err[..err_len as usize].to_vec()).unwrap()),
		}
	}
	.unwrap();

	// key pair from secret key
	let secret: [u8; 32] = [
		184, 80, 22, 77, 31, 238, 200, 105, 138, 204, 163, 41, 148, 124, 152, 133, 189, 29, 148, 3,
		77, 47, 187, 230, 8, 5, 152, 173, 190, 21, 178, 152,
	];
	let key_pair = unsafe {
		let call_key_pair_from_secret_key: Symbol<CallKeyPairFromSecretKey> = lib
			.get(b"_crypto_dsa_custom_key_pair_from_secret_key")
			.unwrap();
		let (mut err, mut err_len) = (vec![0u8; conf.err_len as usize], 0u32 as c_uint);
		let raw = call_key_pair_from_secret_key(
			secret.as_ptr(),
			secret.len() as c_uint,
			err.as_mut_ptr(),
			&mut err_len as *mut c_uint,
		);
		match err_len {
			0 => Ok(raw),
			_ => Err(String::from_utf8(err[..err_len as usize].to_vec()).unwrap()),
		}
	}
	.unwrap();

	// secret key
	let mut out = vec![0u8; conf.secret_len as usize];
	unsafe {
		let call_key_pair_secret_key: Symbol<CallKeyPairSecretKey> =
			lib.get(b"_crypto_dsa_custom_key_pair_secret_key").unwrap();
		call_key_pair_secret_key(key_pair, out.as_mut_ptr(), out.len() as c_uint);
	}

	assert_eq!(out, secret);

	// public key
	let mut public_key = vec![0u8; conf.secret_len as usize];
	unsafe {
		let call_key_pair_public_key: Symbol<CallKeyPairPublicKey> =
			lib.get(b"_crypto_dsa_custom_key_pair_public_key").unwrap();
		call_key_pair_public_key(
			key_pair,
			public_key.as_mut_ptr(),
			public_key.len() as c_uint,
		);
	}

	assert_eq!(
		public_key,
		[
			137, 44, 137, 164, 205, 99, 29, 8, 218, 49, 70, 7, 34, 56, 20, 119, 86, 4, 83, 90, 5,
			245, 14, 149, 157, 33, 32, 157, 1, 116, 14, 186
		]
	);

	// sign
	let message = [97u8, 98, 99];
	let mut signature = vec![0u8; conf.signature_len as usize];
	unsafe {
		let call_key_pair_sign: Symbol<CallKeyPairSign> =
			lib.get(b"_crypto_dsa_custom_key_pair_sign").unwrap();
		call_key_pair_sign(
			key_pair,
			message.as_ptr(),
			message.len() as c_uint,
			signature.as_mut_ptr(),
			signature.len() as c_uint,
		);
	}

	assert_eq!(
		signature.to_vec(),
		vec![
			82, 19, 26, 105, 235, 178, 54, 112, 61, 224, 195, 88, 150, 137, 32, 46, 235, 209, 209,
			108, 64, 153, 12, 58, 216, 179, 88, 38, 49, 167, 162, 103, 219, 116, 93, 187, 145, 86,
			216, 98, 97, 135, 228, 15, 66, 246, 207, 232, 132, 182, 211, 206, 12, 220, 4, 96, 58,
			254, 237, 8, 151, 3, 172, 14
		]
	);

	let verifier = unsafe {
		let call_verifier_from_public_key: Symbol<CallVerifierFromPublicKey> = lib
			.get(b"_crypto_dsa_custom_verifier_from_public_key")
			.unwrap();
		let (mut err, mut err_len) = (vec![0u8; conf.err_len as usize], 0u32 as c_uint);
		let raw = call_verifier_from_public_key(
			public_key.as_ptr(),
			public_key.len() as c_uint,
			err.as_mut_ptr(),
			&mut err_len as *mut c_uint,
		);
		match err_len {
			0 => Ok(raw),
			_ => Err(String::from_utf8(err[..err_len as usize].to_vec()).unwrap()),
		}
	}
	.unwrap();

	// verify
	unsafe {
		let call_verifier_verify: Symbol<CallVerifierVerify> =
			lib.get(b"_crypto_dsa_custom_verifier_verify").unwrap();
		let (mut err, mut err_len) = (vec![0u8; conf.verifier_err_len as usize], 0u32 as c_uint);
		call_verifier_verify(
			verifier,
			message.as_ptr(),
			message.len() as c_uint,
			signature.as_ptr(),
			signature.len() as c_uint,
			err.as_mut_ptr(),
			&mut err_len as *mut c_uint,
		);
		match err_len {
			0 => Ok(()),
			_ => Err(String::from_utf8(err[..err_len as usize].to_vec()).unwrap()),
		}
	}
	.unwrap();

	//free
	unsafe {
		let call_key_pair_free: Symbol<CallKeyPairFree> =
			lib.get(b"_crypto_dsa_custom_verifier_free").unwrap();
		call_key_pair_free(key_pair);
	}
	unsafe {
		let call_verifier_free: Symbol<CallVerifierFree> =
			lib.get(b"_crypto_dsa_custom_verifier_free").unwrap();
		call_verifier_free(verifier);
	}
}
