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

use std::ffi::CString;
use std::os::raw::{c_char, c_uint, c_uchar};

use rust_crypto::blake2b;

use crypto::hash::Hash;
use crypto::KeyLength;

pub struct Blake2b256;

/// A Blake2b256 implementation for sample
impl Hash for Blake2b256 {
	fn name(&self) -> String {
		"blake2b_256".to_string()
	}
	fn key_length(&self) -> KeyLength {
		KeyLength::KeyLength32
	}
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		assert_eq!(out.len(), self.key_length().into());
		blake2b::Blake2b::blake2b(out, data, &[]);
	}
}

declare_custom_lib!(Blake2b256);

#[no_mangle]
pub extern "C" fn _crypto_hash_custom_name() -> *mut c_char {
	let name = Blake2b256.name();
	CString::new(name).expect("qed").into_raw()
}

#[no_mangle]
pub extern "C" fn _crypto_hash_custom_name_free(name: *mut c_char) {
	unsafe {
		assert!(!name.is_null());
		CString::from_raw(name)
	};
}

#[no_mangle]
pub extern "C" fn _crypto_hash_custom_key_length() -> c_uint {
	let length: usize = Blake2b256.key_length().into();
	length as c_uint
}

#[no_mangle]
pub extern "C" fn _crypto_hash_custom_hash(out: *mut c_uchar, out_len: c_uint, data: *const c_uchar, data_len: c_uint) {

	use std::slice;

	let data = unsafe {
		assert!(!data.is_null());
		slice::from_raw_parts(data, data_len as usize)
	};

	let out = unsafe {
		assert!(!out.is_null());
		slice::from_raw_parts_mut(out, out_len as usize)
	};

	Blake2b256.hash(out, data);

}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test() {
		let data = [1u8, 2u8, 3u8];
		let mut out = [0u8; 32];
		Blake2b256.hash(&mut out, &data);

		assert_eq!(
			out,
			[
				17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142,
				220, 157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218
			]
		);
	}
}
