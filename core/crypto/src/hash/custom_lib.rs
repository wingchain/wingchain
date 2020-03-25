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

use std::convert::TryInto;
use std::ffi::CStr;
use std::os::raw::{c_char, c_uchar, c_uint};
use std::path::PathBuf;

#[cfg(unix)]
use libloading::os::unix as imp;
#[cfg(windows)]
use libloading::os::windows as imp;
use libloading::{Library, Symbol};

use crate::hash::Hash;
use crate::{errors, HashLength};

type CallName = unsafe extern "C" fn() -> *mut c_char;
type CallNameFree = unsafe extern "C" fn(*mut c_char);
type CallHashLength = unsafe extern "C" fn() -> c_uint;
type CallHash = unsafe extern "C" fn(*mut c_uchar, c_uint, *const c_uchar, c_uint);

pub struct CustomLib {
	#[allow(dead_code)]
	/// lib is referred by symbols, should be kept
	lib: Library,
	name: String,
	key_length: HashLength,
	call_hash: imp::Symbol<CallHash>,
}

impl CustomLib {
	pub fn new(path: &PathBuf) -> errors::Result<Self> {
		let err = |_| errors::ErrorKind::CustomLibLoadFailed(format!("{:?}", path));

		let lib = Library::new(path).map_err(err)?;

		let (call_name, call_name_free, call_key_length, call_hash) = unsafe {
			let call_name: Symbol<CallName> = lib.get(b"_crypto_hash_custom_name").map_err(err)?;
			let call_name = call_name.into_raw();

			let call_name_free: Symbol<CallNameFree> =
				lib.get(b"_crypto_hash_custom_name_free").map_err(err)?;
			let call_name_free = call_name_free.into_raw();

			let call_key_length: Symbol<CallHashLength> =
				lib.get(b"_crypto_hash_custom_key_length").map_err(err)?;
			let call_key_length = call_key_length.into_raw();

			let call_hash: Symbol<CallHash> = lib.get(b"_crypto_hash_custom_hash").map_err(err)?;
			let call_hash = call_hash.into_raw();

			(call_name, call_name_free, call_key_length, call_hash)
		};

		let name = Self::name(&call_name, &call_name_free, &path)?;
		let key_length = Self::key_length(&call_key_length)?;

		Ok(CustomLib {
			lib,
			name,
			key_length,
			call_hash,
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

	fn key_length(call_key_length: &imp::Symbol<CallHashLength>) -> errors::Result<HashLength> {
		let key_length: usize = unsafe {
			let key_length = call_key_length();
			key_length as usize
		};

		key_length.try_into()
	}
}

impl Hash for CustomLib {
	fn name(&self) -> String {
		self.name.clone()
	}

	fn key_length(&self) -> HashLength {
		self.key_length.clone()
	}
	fn hash(&self, out: &mut [u8], data: &[u8]) {
		unsafe {
			(self.call_hash)(
				out.as_mut_ptr(),
				out.len() as c_uint,
				data.as_ptr(),
				data.len() as c_uint,
			);
		};
	}
}

#[macro_export]
macro_rules! declare_hash_custom_lib {
	($impl:path) => {
		use std::ffi::CString;
		use std::os::raw::{c_char, c_uchar, c_uint};

		#[no_mangle]
		pub extern "C" fn _crypto_hash_custom_name() -> *mut c_char {
			let name = $impl.name();
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
			let length: usize = $impl.key_length().into();
			length as c_uint
		}

		#[no_mangle]
		pub extern "C" fn _crypto_hash_custom_hash(
			out: *mut c_uchar,
			out_len: c_uint,
			data: *const c_uchar,
			data_len: c_uint,
		) {
			use std::slice;

			let data = unsafe {
				assert!(!data.is_null());
				slice::from_raw_parts(data, data_len as usize)
			};

			let out = unsafe {
				assert!(!out.is_null());
				slice::from_raw_parts_mut(out, out_len as usize)
			};

			$impl.hash(out, data);
		}
	};
}
