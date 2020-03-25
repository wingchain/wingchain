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

use std::os::raw::{c_char, c_uchar, c_uint};
use std::path::PathBuf;

#[cfg(unix)]
use libloading::os::unix as imp;
#[cfg(windows)]
use libloading::os::windows as imp;
use libloading::{Library, Symbol};

use crate::dsa::{Dsa, KeyPair, Verifier};
use crate::errors;
use std::ffi::CStr;
use std::rc::Rc;

pub trait CDsa {
	const ERR_LEN: usize;
	type KeyPair: CKeyPair;
	type Verifier: CVerifier;

	fn name(&self) -> String;

	fn generate_key_pair(&self) -> Result<Self::KeyPair, Vec<u8>>;

	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> Result<Self::KeyPair, Vec<u8>>;

	fn verifier_from_public_key(&self, public_key: &[u8]) -> Result<Self::Verifier, Vec<u8>>;
}

pub trait CKeyPair {
	const PUBLIC_LEN: usize;
	const SECRET_LEN: usize;
	const SIGNATURE_LEN: usize;
	fn public_key(&self, out: &mut [u8]);
	fn secret_key(&self, out: &mut [u8]);
	fn sign(&self, message: &[u8], out: &mut [u8]);
}

pub trait CVerifier {
	const ERR_LEN: usize;
	fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Vec<u8>>;
}

#[repr(C)]
pub struct CDsaConf {
	pub err_len: c_uint,
	pub public_len: c_uint,
	pub secret_len: c_uint,
	pub signature_len: c_uint,
	pub verifier_err_len: c_uint,
}

#[repr(C)]
pub struct TKeyPair {
	_unused: [u8; 0],
	call: Rc<CallKeyPair>,
}

#[repr(C)]
pub struct TVerifier {
	_unused: [u8; 0],
	call: Rc<CallVerifier>,
}

type CallConf = unsafe extern "C" fn() -> CDsaConf;
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

pub struct CustomLib {
	#[allow(dead_code)]
	/// lib is referred by symbols, should be kept
	lib: Library,
	name: String,
	err_len: usize,
	public_len: usize,
	secret_len: usize,
	signature_len: usize,
	verifier_err_len: usize,
	call_generate_key_pair: imp::Symbol<CallGenerateKeyPair>,
	call_key_pair_from_secret_key: imp::Symbol<CallKeyPairFromSecretKey>,
	call_verifier_from_public_key: imp::Symbol<CallVerifierFromPublicKey>,
	call_key_pair: Rc<CallKeyPair>,
	call_verifier: Rc<CallVerifier>,
}

struct CallKeyPair {
	call_key_pair_secret_key: imp::Symbol<CallKeyPairSecretKey>,
	call_key_pair_public_key: imp::Symbol<CallKeyPairPublicKey>,
	call_key_pair_sign: imp::Symbol<CallKeyPairSign>,
	call_key_pair_free: imp::Symbol<CallKeyPairFree>,
}

struct CallVerifier {
	call_verifier_verify: imp::Symbol<CallVerifierVerify>,
	call_verifier_free: imp::Symbol<CallVerifierFree>,
}

impl CustomLib {
	pub fn new(path: &PathBuf) -> errors::Result<Self> {
		let err = |_| errors::ErrorKind::CustomLibLoadFailed(format!("{:?}", path));

		let lib = Library::new(path).map_err(err)?;

		let (
			call_name,
			call_name_free,
			call_conf,
			call_generate_key_pair,
			call_key_pair_from_secret_key,
			call_key_pair_secret_key,
			call_key_pair_public_key,
			call_key_pair_sign,
			call_key_pair_free,
			call_verifier_from_public_key,
			call_verifier_verify,
			call_verifier_free,
		) = unsafe {
			let call_name: Symbol<CallName> = lib.get(b"_crypto_dsa_custom_name").map_err(err)?;
			let call_name = call_name.into_raw();

			let call_conf: Symbol<CallConf> = lib.get(b"_crypto_dsa_custom_conf").map_err(err)?;
			let call_conf = call_conf.into_raw();
			let call_name_free: Symbol<CallNameFree> =
				lib.get(b"_crypto_dsa_custom_name_free").map_err(err)?;
			let call_name_free = call_name_free.into_raw();

			let call_generate_key_pair: Symbol<CallGenerateKeyPair> = lib
				.get(b"_crypto_dsa_custom_generate_key_pair")
				.map_err(err)?;
			let call_generate_key_pair = call_generate_key_pair.into_raw();
			let call_key_pair_from_secret_key: Symbol<CallKeyPairFromSecretKey> = lib
				.get(b"_crypto_dsa_custom_key_pair_from_secret_key")
				.map_err(err)?;
			let call_key_pair_from_secret_key = call_key_pair_from_secret_key.into_raw();
			let call_key_pair_secret_key: Symbol<CallKeyPairSecretKey> = lib
				.get(b"_crypto_dsa_custom_key_pair_secret_key")
				.map_err(err)?;
			let call_key_pair_secret_key = call_key_pair_secret_key.into_raw();
			let call_key_pair_public_key: Symbol<CallKeyPairPublicKey> = lib
				.get(b"_crypto_dsa_custom_key_pair_public_key")
				.map_err(err)?;
			let call_key_pair_public_key = call_key_pair_public_key.into_raw();
			let call_key_pair_sign: Symbol<CallKeyPairSign> =
				lib.get(b"_crypto_dsa_custom_key_pair_sign").map_err(err)?;
			let call_key_pair_sign = call_key_pair_sign.into_raw();
			let call_key_pair_free: Symbol<CallKeyPairFree> =
				lib.get(b"_crypto_dsa_custom_key_pair_free").map_err(err)?;
			let call_key_pair_free = call_key_pair_free.into_raw();
			let call_verifier_from_public_key: Symbol<CallVerifierFromPublicKey> = lib
				.get(b"_crypto_dsa_custom_verifier_from_public_key")
				.map_err(err)?;
			let call_verifier_from_public_key = call_verifier_from_public_key.into_raw();
			let call_verifier_verify: Symbol<CallVerifierVerify> = lib
				.get(b"_crypto_dsa_custom_verifier_verify")
				.map_err(err)?;
			let call_verifier_verify = call_verifier_verify.into_raw();
			let call_verifier_free: Symbol<CallVerifierFree> =
				lib.get(b"_crypto_dsa_custom_verifier_free").map_err(err)?;
			let call_verifier_free = call_verifier_free.into_raw();

			(
				call_name,
				call_name_free,
				call_conf,
				call_generate_key_pair,
				call_key_pair_from_secret_key,
				call_key_pair_secret_key,
				call_key_pair_public_key,
				call_key_pair_sign,
				call_key_pair_free,
				call_verifier_from_public_key,
				call_verifier_verify,
				call_verifier_free,
			)
		};

		let name = Self::name(&call_name, &call_name_free, &path)?;
		let conf = Self::conf(&call_conf)?;

		let call_key_pair = Rc::new(CallKeyPair {
			call_key_pair_secret_key,
			call_key_pair_public_key,
			call_key_pair_sign,
			call_key_pair_free,
		});

		let call_verifier = Rc::new(CallVerifier {
			call_verifier_verify,
			call_verifier_free,
		});

		Ok(CustomLib {
			lib,
			name,
			err_len: conf.err_len as usize,
			public_len: conf.public_len as usize,
			secret_len: conf.secret_len as usize,
			signature_len: conf.signature_len as usize,
			verifier_err_len: conf.verifier_err_len as usize,
			call_generate_key_pair,
			call_key_pair_from_secret_key,
			call_key_pair,
			call_verifier_from_public_key,
			call_verifier,
		})
	}

	fn name(
		call_name: &imp::Symbol<CallName>,
		call_name_free: &imp::Symbol<CallNameFree>,
		path: &PathBuf,
	) -> errors::Result<String> {
		let err = |_| errors::ErrorKind::InvalidName(format!("{:?}", path));

		let name: String = unsafe {
			let raw = call_name();
			let name = CStr::from_ptr(raw).to_str().map_err(err)?;
			let name = name.to_owned();
			call_name_free(raw);
			name
		};
		Ok(name)
	}

	fn conf(call_conf: &imp::Symbol<CallConf>) -> errors::Result<CDsaConf> {
		let conf: CDsaConf = unsafe {
			let conf = call_conf();
			conf
		};
		Ok(conf)
	}
}

pub struct CustomKeyPair {
	inner: *mut TKeyPair,
	call: Rc<CallKeyPair>,
	public_len: usize,
	secret_len: usize,
	signature_len: usize,
}

impl Drop for CustomKeyPair {
	fn drop(&mut self) {
		unsafe {
			(self.call.call_key_pair_free)(self.inner);
		}
	}
}

pub struct CustomVerifier {
	inner: *mut TVerifier,
	call: Rc<CallVerifier>,
	verifier_err_len: usize,
}

impl Drop for CustomVerifier {
	fn drop(&mut self) {
		unsafe {
			(self.call.call_verifier_free)(self.inner);
		}
	}
}

impl Dsa for CustomLib {
	type Error = errors::Error;
	type KeyPair = CustomKeyPair;
	type Verifier = CustomVerifier;

	fn name(&self) -> String {
		self.name.clone()
	}

	fn generate_key_pair(&self) -> Result<Self::KeyPair, Self::Error> {
		let (mut err, mut err_len) = (vec![0u8; self.err_len as usize], 0u32 as c_uint);
		unsafe {
			let raw = (self.call_generate_key_pair)(err.as_mut_ptr(), &mut err_len as *mut c_uint);
			match err_len {
				0 => Ok(CustomKeyPair {
					inner: raw,
					call: self.call_key_pair.clone(),
					public_len: self.public_len,
					secret_len: self.secret_len,
					signature_len: self.signature_len,
				}),
				_ => Err(build_err(err, err_len).into()),
			}
		}
	}

	fn key_pair_from_secret_key(&self, secret_key: &[u8]) -> Result<Self::KeyPair, Self::Error> {
		let (mut err, mut err_len) = (vec![0u8; self.err_len as usize], 0u32 as c_uint);
		unsafe {
			let raw = (self.call_key_pair_from_secret_key)(
				secret_key.as_ptr(),
				secret_key.len() as c_uint,
				err.as_mut_ptr(),
				&mut err_len as *mut c_uint,
			);
			match err_len {
				0 => Ok(CustomKeyPair {
					inner: raw,
					call: self.call_key_pair.clone(),
					public_len: self.public_len,
					secret_len: self.secret_len,
					signature_len: self.signature_len,
				}),
				_ => Err(build_err(err, err_len).into()),
			}
		}
	}

	fn verifier_from_public_key(&self, public_key: &[u8]) -> Result<Self::Verifier, Self::Error> {
		let (mut err, mut err_len) = (vec![0u8; self.err_len as usize], 0u32 as c_uint);
		unsafe {
			let raw = (self.call_verifier_from_public_key)(
				public_key.as_ptr(),
				public_key.len() as c_uint,
				err.as_mut_ptr(),
				&mut err_len as *mut c_uint,
			);
			match err_len {
				0 => Ok(CustomVerifier {
					inner: raw,
					call: self.call_verifier.clone(),
					verifier_err_len: self.verifier_err_len,
				}),
				_ => Err(build_err(err, err_len).into()),
			}
		}
	}
}

impl KeyPair for CustomKeyPair {
	fn public_key(&self) -> Vec<u8> {
		let mut public_key = vec![0u8; self.public_len as usize];
		unsafe {
			(self.call.call_key_pair_public_key)(
				self.inner,
				public_key.as_mut_ptr(),
				public_key.len() as c_uint,
			);
		}
		public_key.to_vec()
	}
	fn secret_key(&self) -> Vec<u8> {
		let mut secret_key = vec![0u8; self.secret_len as usize];
		unsafe {
			(self.call.call_key_pair_secret_key)(
				self.inner,
				secret_key.as_mut_ptr(),
				secret_key.len() as c_uint,
			);
		}
		secret_key.to_vec()
	}
	fn sign(&self, message: &[u8]) -> Vec<u8> {
		let mut signature = vec![0u8; self.signature_len as usize];
		unsafe {
			(self.call.call_key_pair_sign)(
				self.inner,
				message.as_ptr(),
				message.len() as c_uint,
				signature.as_mut_ptr(),
				signature.len() as c_uint,
			);
		}
		signature
	}
}

impl Verifier for CustomVerifier {
	type Error = errors::Error;
	fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Self::Error> {
		let (mut err, mut err_len) = (vec![0u8; self.verifier_err_len as usize], 0u32 as c_uint);
		unsafe {
			(self.call.call_verifier_verify)(
				self.inner,
				message.as_ptr(),
				message.len() as c_uint,
				signature.as_ptr(),
				signature.len() as c_uint,
				err.as_mut_ptr(),
				&mut err_len as *mut c_uint,
			);
			match err_len {
				0 => Ok(()),
				_ => Err(build_err(err, err_len).into()),
			}
		}
	}
}

#[inline]
fn build_err(err: Vec<u8>, err_len: c_uint) -> String {
	String::from_utf8(err[..err_len as usize].to_vec()).unwrap_or("Unknown".to_string())
}

#[macro_export]
macro_rules! declare_dsa_custom_lib {
	($impl:path) => {
		use std::ffi::CString;
		use std::os::raw::{c_char, c_uchar, c_uint};
		use std::ptr::null_mut;
		use std::slice;

		#[no_mangle]
		pub extern "C" fn _crypto_dsa_custom_conf() -> CDsaConf {
			CDsaConf {
				err_len: <$impl as CDsa>::ERR_LEN as c_uint,
				public_len: <$impl as CDsa>::KeyPair::PUBLIC_LEN as c_uint,
				secret_len: <$impl as CDsa>::KeyPair::SECRET_LEN as c_uint,
				signature_len: <$impl as CDsa>::KeyPair::SIGNATURE_LEN as c_uint,
				verifier_err_len: <$impl as CDsa>::Verifier::ERR_LEN as c_uint,
			}
		}

		#[no_mangle]
		pub extern "C" fn _crypto_dsa_custom_name() -> *mut c_char {
			let name = $impl.name();
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
			let key_pair = match $impl.generate_key_pair() {
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
			let key_pair = match $impl.key_pair_from_secret_key(secret_key) {
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
		pub extern "C" fn _crypto_dsa_custom_key_pair_free(key_pair: *mut KeyPair) {
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
			let verifier = match $impl.verifier_from_public_key(public_key) {
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
					crypto_dsa_error_handle(e, err, err_len);
				}
			}
			std::mem::forget(verifier);
		}

		#[no_mangle]
		pub extern "C" fn _crypto_dsa_custom_verifier_free(verifier: *mut Verifier) {
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
	};
}
