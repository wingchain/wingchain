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

use libloading::{Library, Symbol};
#[cfg(unix)]
use libloading::os::unix as imp;

use crate::{errors, KeyLength};

#[cfg(windows)]
use self::os::windows as imp;
use crate::hash::Hash;
use std::ffi::CStr;
use std::convert::TryInto;

type CallName = unsafe extern "C" fn() -> *mut c_char;
type CallNameFree = unsafe extern "C" fn(*mut c_char);
type CallKeyLength = unsafe extern "C" fn() -> usize;
type CallHash = unsafe extern "C" fn(*mut c_uchar, c_uint, *const c_uchar, c_uint);

pub struct CustomLib {
	#[allow(dead_code)]
	/// symbols refer lib, should keep lib
	lib: Library,
	name: imp::Symbol<CallName>,
	name_free: imp::Symbol<CallNameFree>,
	key_length: imp::Symbol<CallKeyLength>,
	hash: imp::Symbol<CallHash>,
}

impl CustomLib {
	pub fn new(path: &PathBuf) -> errors::Result<Self> {
		let err = |_| { errors::ErrorKind::CustomLibLoadFailed(format!("{:?}", path)) };

		let lib = Library::new(path)
			.map_err(err)?;

		let (name, name_free, key_length, hash) = unsafe {
			let call_name: Symbol<CallName> = lib.get(b"_crypto_hash_custom_name").map_err(err)?;
			let call_name = call_name.into_raw();

			let call_name_free: Symbol<CallNameFree> = lib.get(b"_crypto_hash_custom_name_free").map_err(err)?;
			let call_name_free = call_name_free.into_raw();

			let call_key_length: Symbol<CallKeyLength> = lib.get(b"_crypto_hash_custom_key_length").map_err(err)?;
			let call_key_length = call_key_length.into_raw();

			let call_hash: Symbol<CallHash> = lib.get(b"_crypto_hash_custom_hash").map_err(err)?;
			let call_hash = call_hash.into_raw();

			(call_name, call_name_free, call_key_length, call_hash)
		};
		Ok(CustomLib {
			lib,
			name,
			name_free,
			key_length,
			hash,
		})
	}
}

impl Hash for CustomLib {
	fn name(&self) -> String {
		let name: String = unsafe {
			let raw = (self.name)();
			let name = CStr::from_ptr(raw).to_str().expect("").to_owned();
			(self.name_free)(raw);
			name
		};
		name
	}
	fn key_length(&self) -> KeyLength {
		let key_length: usize = unsafe {
			let key_length = (self.key_length)();
			key_length as usize
		};

		key_length.try_into().expect("qed")
	}
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		unsafe {
			(self.hash)(out.as_mut_ptr(), out.len() as c_uint, data.as_ptr(), data.len() as c_uint);
		};
	}
}
