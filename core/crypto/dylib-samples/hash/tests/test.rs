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

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uchar, c_uint};
use std::str::FromStr;

use libloading::{Library, Symbol};

use crypto::hash::{Hash, HashImpl};
use crypto::HashLength;

#[test]
fn test_custom_lib_hash() {
	let path = utils_test::get_dylib("crypto_dylib_samples_hash");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let hasher = HashImpl::from_str(&path).unwrap();

	let name = hasher.name();
	assert_eq!(name, "blake2b_160".to_string());

	let length = hasher.length();
	assert_eq!(length, HashLength::HashLength20);

	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 20];
	hasher.hash(&mut out, &data);
	assert_eq!(
		out,
		[
			197, 117, 145, 134, 122, 108, 242, 5, 233, 74, 212, 142, 167, 139, 236, 142, 103, 194,
			14, 98
		]
	);
}

#[test]
fn test_dylib_hash() {
	let path = utils_test::get_dylib("crypto_dylib_samples_hash");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let lib = Library::new(path).unwrap();
	type CallName = unsafe extern "C" fn() -> *mut c_char;
	type CallNameFree = unsafe extern "C" fn(*mut c_char);

	// name
	let name: String = unsafe {
		let call_name: Symbol<CallName> = lib.get(b"_crypto_hash_custom_name").unwrap();
		let call_name_free: Symbol<CallNameFree> =
			lib.get(b"_crypto_hash_custom_name_free").unwrap();
		let raw = call_name();
		let name = CStr::from_ptr(raw).to_str().expect("").to_owned();
		call_name_free(raw);
		name
	};

	assert_eq!(name, "blake2b_160");

	// key length
	type CallHashLength = unsafe extern "C" fn() -> usize;

	let length: usize = unsafe {
		let call_length: Symbol<CallHashLength> = lib.get(b"_crypto_hash_custom_length").unwrap();
		let length = call_length();
		length as usize
	};

	assert_eq!(length, 20);

	// hash
	type CallHash = unsafe extern "C" fn(*mut c_uchar, c_uint, *const c_uchar, c_uint);

	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 20];

	unsafe {
		let call_hash: Symbol<CallHash> = lib.get(b"_crypto_hash_custom_hash").unwrap();
		call_hash(
			out.as_mut_ptr(),
			out.len() as c_uint,
			data.as_ptr(),
			data.len() as c_uint,
		);
	};

	assert_eq!(
		out,
		[
			197, 117, 145, 134, 122, 108, 242, 5, 233, 74, 212, 142, 167, 139, 236, 142, 103, 194,
			14, 98
		]
	);
}

#[test]
fn test_c_string_raw_pointer() {
	let raw = get_str();

	let c_str = unsafe { CStr::from_ptr(raw) };

	let str1 = c_str.to_str().unwrap().to_owned();

	let str2 = c_str.to_str().unwrap();

	free_str(raw);

	// known value, copied before freeing
	assert_eq!(str1, "test");

	// unknown value, freed
	assert_ne!(str2, "test");
}

#[test]
fn test_c_string_raw_pointer2() {
	let raw = get_str();

	let string = get_string(raw);

	assert_eq!(string, "test".to_string());

	move_string(string);

	// should not use raw again:
	// let string = get_string(raw);
}

fn move_string(_str: String) {
	//will free str
}

fn get_string(raw: *mut i8) -> String {
	let c_string = unsafe { CString::from_raw(raw) };

	c_string.into_string().unwrap()
}

fn get_str() -> *mut c_char {
	let s = CString::new("test").unwrap();
	let s = s.into_raw();
	s
}

fn free_str(s: *mut c_char) {
	unsafe {
		if s.is_null() {
			return;
		}
		CString::from_raw(s)
	};
}
