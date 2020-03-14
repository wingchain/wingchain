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

use std::path::PathBuf;
use std::str::FromStr;

use error_chain::bail;
use libloading::{Library, Symbol};

use blake2b::Blake2b256;
use sm3::SM3;

use crate::errors;
use crate::hash::blake2b::Blake2b160;
use crate::KeyLength;
use std::os::raw::{c_char, c_uchar, c_uint};
use std::ffi::CStr;
use std::convert::TryInto;

pub mod blake2b;
pub mod sm3;

pub trait Hash {
	fn name(&self) -> String;
	fn key_length(&self) -> KeyLength;
	fn hash(&self, out: &mut [u8], data: &[u8]);
}

pub enum HashImpl {
	Blake2b256,
	Blake2b160,
	SM3,
	/// custom hash impl provided by dylib
	Custom(CustomLib),
}

pub struct CustomLib(Library);

impl Hash for HashImpl {
	#[inline]
	fn name(&self) -> String {
		match self {
			Self::Blake2b256 => Blake2b256.name(),
			Self::Blake2b160 => Blake2b160.name(),
			Self::SM3 => SM3.name(),
			Self::Custom(custom) => get_name_from_lib(&custom.0)
		}
	}
	#[inline]
	fn key_length(&self) -> KeyLength {
		match self {
			Self::Blake2b256 => Blake2b256.key_length(),
			Self::Blake2b160 => Blake2b160.key_length(),
			Self::SM3 => SM3.key_length(),
			Self::Custom(custom) => get_key_length_from_lib(&custom.0),
		}
	}
	#[inline]
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		match self {
			Self::Blake2b256 => Blake2b256.hash(out, data),
			Self::Blake2b160 => Blake2b160.hash(out, data),
			Self::SM3 => SM3.hash(out, data),
			Self::Custom(custom) => hash_from_lib(&custom.0, out, data),
		}
	}
}

impl FromStr for HashImpl {
	type Err = errors::Error;
	#[inline]
	fn from_str(s: &str) -> Result<HashImpl, Self::Err> {
		match s {
			"blake2b_256" => Ok(HashImpl::Blake2b256),
			"blake2b_160" => Ok(HashImpl::Blake2b160),
			"sm3" => Ok(HashImpl::SM3),
			other => {
				let path = PathBuf::from(&other);
				let lib = load_custom_lib(&path)?;
				let custom_lib = CustomLib(lib);
				Ok(HashImpl::Custom(custom_lib))
			}
		}
	}
}

#[macro_export]
macro_rules! declare_custom_lib {
	($impl:path) => {
		#[no_mangle]
		pub extern "C" fn _crypto_hash_create() -> *mut dyn Hash {
			let boxed: Box<dyn Hash> = Box::new($impl);
			Box::into_raw(boxed)
		}
	};
}

fn load_custom_lib(path: &PathBuf) -> errors::Result<Library> {
	if !path.exists() {
		bail!(errors::ErrorKind::CustomLibNotFound(format!("{:?}", path)));
	}

	let lib = Library::new(path)
		.map_err(|_| errors::ErrorKind::CustomLibLoadFailed(format!("{:?}", path)))?;

	Ok(lib)
}

fn get_name_from_lib(lib: &Library) -> String {
	type CallName = unsafe extern "C" fn() -> *mut c_char;
	type CallNameFree = unsafe extern "C" fn(*mut c_char);

	let name: String = unsafe {
		let call_name: Symbol<CallName> = lib.get(b"_crypto_hash_custom_name").unwrap();
		let call_name_free: Symbol<CallNameFree> = lib.get(b"_crypto_hash_custom_name_free").unwrap();
		let raw = call_name();
		let name = CStr::from_ptr(raw).to_str().expect("").to_owned();
		call_name_free(raw);
		name
	};
	name
}

fn get_key_length_from_lib(lib: &Library) -> KeyLength {
	type CallKeyLength = unsafe extern "C" fn() -> usize;

	let key_length: usize = unsafe {
		let call_key_length: Symbol<CallKeyLength> = lib.get(b"_crypto_hash_custom_key_length").unwrap();
		let key_length = call_key_length();
		key_length as usize
	};

	key_length.try_into().expect("qed")
}

fn hash_from_lib(lib: &Library, out: &mut [u8], data: &[u8]) {
	type CallHash = unsafe extern "C" fn(*mut c_uchar, c_uint, *const c_uchar, c_uint);

	unsafe {
		let call_hash: Symbol<CallHash> = lib.get(b"_crypto_hash_custom_hash").unwrap();
		call_hash(out.as_mut_ptr(), out.len() as c_uint, data.as_ptr(), data.len() as c_uint);
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test() {
		let hash = HashImpl::Blake2b256;
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		hash.hash(&mut out, &data);
		assert_eq!(
			out,
			[
				17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142,
				220, 157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
			]
		);
	}

	#[test]
	fn test_from_str_blake2b_256() {
		let hash = HashImpl::from_str("blake2b_256").unwrap();
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		hash.hash(&mut out, &data);
		assert_eq!(
			out,
			[
				17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142,
				220, 157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
			]
		);
	}

	#[test]
	fn test_from_str_blake2b_160() {
		let hash = HashImpl::from_str("blake2b_160").unwrap();
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 20];
		hash.hash(&mut out, &data);
		assert_eq!(
			out,
			[
				197, 117, 145, 134, 122, 108, 242, 5, 233, 74, 212, 142, 167, 139, 236, 142, 103,
				194, 14, 98
			]
		);
	}

	#[test]
	fn test_from_str_sm3() {
		let hash = HashImpl::from_str("sm3").unwrap();
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		hash.hash(&mut out, &data);
		assert_eq!(
			out,
			[
				158, 139, 109, 83, 238, 96, 25, 26, 219, 93, 71, 130, 155, 7, 70, 50, 56, 171, 15,
				159, 227, 157, 222, 97, 216, 238, 73, 54, 50, 158, 49, 251
			]
		);
	}
}
