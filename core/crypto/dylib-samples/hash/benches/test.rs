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
#![feature(test)]

extern crate test;

use std::str::FromStr;
use test::{black_box, Bencher};

use crypto::hash::{Hash, HashImpl};
use std::ffi::CString;

#[bench]
fn bench_hash_native(b: &mut Bencher) {
	let hash = HashImpl::Blake2b160;

	let data = (0..32u8).collect::<Vec<_>>();
	let mut out = [0u8; 20];

	b.iter(|| black_box(hash.hash(&mut out, &data)));
}

#[bench]
fn bench_hash_dylib(b: &mut Bencher) {
	let path = utils_test::get_dylib("crypto_dylib_samples_hash");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let hasher = HashImpl::from_str(&path).unwrap();

	let data = (0..32u8).collect::<Vec<_>>();
	let mut out = [0u8; 20];

	b.iter(|| black_box(hasher.hash(&mut out, &data)));
}

#[bench]
fn bench_name_native(b: &mut Bencher) {
	let hash = HashImpl::Blake2b160;

	b.iter(|| black_box(hash.name()));
}

#[bench]
fn bench_name_dylib(b: &mut Bencher) {
	let path = utils_test::get_dylib("crypto_dylib_samples_hash");

	assert!(
		path.exists(),
		"should build --release first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let hasher = HashImpl::from_str(&path).unwrap();

	b.iter(|| black_box(hasher.name()));
}

#[bench]
fn bench_key_length_native(b: &mut Bencher) {
	let hash = HashImpl::Blake2b160;

	b.iter(|| black_box(hash.length()));
}

#[bench]
fn bench_key_length_dylib(b: &mut Bencher) {
	let path = utils_test::get_dylib("crypto_dylib_samples_hash");

	assert!(
		path.exists(),
		"should build --release first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let hasher = HashImpl::from_str(&path).unwrap();

	b.iter(|| black_box(hasher.length()));
}

#[bench]
fn bench_string(b: &mut Bencher) {
	let get_name = || "test".to_string();

	b.iter(|| black_box(get_name()));
}

/// to run with dylib, should `cargo +nightly build --release` first.
#[bench]
fn bench_string_ffi(b: &mut Bencher) {
	let get_name = || {
		let s = CString::new("test").unwrap().into_raw();
		unsafe { CString::from_raw(s).into_string().unwrap() }
	};

	b.iter(|| black_box(get_name()));
}
