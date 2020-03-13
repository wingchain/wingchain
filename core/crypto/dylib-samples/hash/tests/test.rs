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

use assert_cmd::cargo::cargo_bin;
use libloading::{Library, Symbol};

use crypto::hash::{Hash, HashImpl};
use std::str::FromStr;

#[test]
fn test_load_dylib() {
	let ext = get_dylib_ext();

	let path = cargo_bin(format!("libcrypto_dylib_samples_hash.{}", ext));

	// in case no build first
	if !path.exists() {
		return;
	}

	let lib = Library::new(path).unwrap();
	type Constructor = unsafe fn() -> *mut dyn Hash;

	let hasher: Box<dyn Hash> = unsafe {
		let constructor: Symbol<Constructor> = lib.get(b"_crypto_hash_create").unwrap();
		let boxed_raw = constructor();
		let hash = Box::from_raw(boxed_raw);
		hash
	};

	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 20];
	hasher.hash(&mut out, &data);
	assert_eq!(
		out,
		[112, 55, 128, 113, 152, 194, 42, 125, 43, 8, 7, 55, 29, 118, 55, 121, 168, 79, 223, 207]
	);
}

#[test]
fn test_custom_lib() {
	let ext = get_dylib_ext();

	let path = cargo_bin(format!("libcrypto_dylib_samples_hash.{}", ext));

	// in case no build first
	if !path.exists() {
		return;
	}

	let path = path.to_string_lossy();
	let hasher = HashImpl::from_str(&path).unwrap();

	let data = [1u8, 2u8, 3u8];
	let mut out = [0u8; 20];
	hasher.hash(&mut out, &data);
	assert_eq!(
		out,
		[112, 55, 128, 113, 152, 194, 42, 125, 43, 8, 7, 55, 29, 118, 55, 121, 168, 79, 223, 207]
	);
}

#[cfg(target_os = "macos")]
fn get_dylib_ext() -> &'static str {
	"dylib"
}

#[cfg(target_os = "linux")]
fn get_dylib_ext() -> &'static str {
	"so"
}

#[cfg(target_os = "windows")]
fn get_dylib_ext() -> &'static str {
	"dll"
}
