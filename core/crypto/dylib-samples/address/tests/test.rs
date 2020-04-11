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

use std::str::FromStr;

use crypto::address::{Address, AddressImpl};
use crypto::AddressLength;

#[test]
fn test_custom_lib_address() {
	let path = utils_test::get_dylib("crypto_dylib_samples_address");

	assert!(
		path.exists(),
		"should build first to make exist: {:?}",
		path
	);

	let path = path.to_string_lossy();
	let address = AddressImpl::from_str(&path).unwrap();

	let name = address.name();
	assert_eq!(name, "blake2b_160".to_string());

	let length = address.length();
	assert_eq!(length, AddressLength::AddressLength20);

	let data = (0u8..32).collect::<Vec<_>>();
	let mut out = [0u8; 20];
	address.address(&mut out, &data);
	assert_eq!(
		out,
		[
			177, 177, 51, 185, 159, 81, 110, 108, 130, 206, 218, 137, 46, 245, 175, 80, 250, 75,
			78, 113
		]
	);
}
