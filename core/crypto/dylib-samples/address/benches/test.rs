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

use crypto::address::{Address, AddressImpl};
use std::ffi::CString;

#[bench]
fn bench_address_native(b: &mut Bencher) {
	let address = AddressImpl::Blake2b160;

	let data = (0..32u8).collect::<Vec<_>>();
	let mut out = [0u8; 20];

	b.iter(|| black_box(address.address(&mut out, &data)));
}

#[bench]
fn bench_address_dylib(b: &mut Bencher) {
	let path = utils::get_dylib("crypto_dylib_samples_address");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let addresser = AddressImpl::from_str(&path).unwrap();

	let data = (0..32u8).collect::<Vec<_>>();
	let mut out = [0u8; 20];

	b.iter(|| black_box(addresser.address(&mut out, &data)));
}
