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
use std::sync::Arc;

use assert_cmd::cargo::cargo_bin;

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{Dsa, DsaImpl, KeyPair, KeyPairImpl};
use primitives::{Address, PublicKey, SecretKey};

#[cfg(target_os = "macos")]
pub fn get_dylib(package_name: &str) -> PathBuf {
	cargo_bin(format!("lib{}.dylib", package_name))
}

#[cfg(target_os = "linux")]
pub fn get_dylib(package_name: &str) -> PathBuf {
	cargo_bin(format!("lib{}.so", package_name))
}

#[cfg(target_os = "windows")]
pub fn get_dylib(package_name: &str) -> PathBuf {
	let path = cargo_bin(format!("{}.dll", package_name));
	let path = path.to_string_lossy();
	let path = path.trim_end_matches(".exe");
	PathBuf::from(path)
}

#[derive(Clone)]
pub struct TestAccount {
	pub secret_key: SecretKey,
	pub public_key: PublicKey,
	pub key_pair: Arc<KeyPairImpl>,
	pub address: Address,
}

pub fn test_accounts(dsa: Arc<DsaImpl>, address: Arc<AddressImpl>) -> Vec<TestAccount> {
	let (secret_key_len, public_key_len, _) = dsa.length().into();
	let address_len = address.length().into();

	let mut list = vec![];

	for i in 1..=8 {
		let account = {
			let secret_key = SecretKey(vec![i; secret_key_len]);

			let key_pair = dsa.key_pair_from_secret_key(&secret_key.0).unwrap();
			let public_key = PublicKey({
				let mut out = vec![0u8; public_key_len];
				key_pair.public_key(&mut out);
				out
			});
			let address = Address({
				let mut out = vec![0u8; address_len];
				address.address(&mut out, &public_key.0);
				out
			});
			TestAccount {
				secret_key,
				public_key,
				key_pair: Arc::new(key_pair),
				address,
			}
		};
		list.push(account);
	}

	list
}
