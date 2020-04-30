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

pub fn test_accounts(
	dsa: Arc<DsaImpl>,
	address: Arc<AddressImpl>,
) -> (
	(SecretKey, PublicKey, KeyPairImpl, Address),
	(SecretKey, PublicKey, KeyPairImpl, Address),
) {
	let (secret_key_len, public_key_len, _) = dsa.length().into();
	let address_len = address.length().into();

	let account1 = {
		let secret_key = SecretKey(vec![1u8; secret_key_len]);

		let key_pair = dsa.key_pair_from_secret_key(&secret_key.0).unwrap();
		let public_key = PublicKey({
			let mut out = vec![0u8; public_key_len];
			key_pair.public_key(&mut out);
			out
		});
		let account = Address({
			let mut out = vec![0u8; address_len];
			address.address(&mut out, &public_key.0);
			out
		});
		(secret_key, public_key, key_pair, account)
	};

	let account2 = {
		let secret_key = SecretKey(vec![2u8; secret_key_len]);

		let key_pair = dsa.key_pair_from_secret_key(&secret_key.0).unwrap();
		let public_key = PublicKey({
			let mut out = vec![0u8; public_key_len];
			key_pair.public_key(&mut out);
			out
		});
		let account = Address({
			let mut out = vec![0u8; address_len];
			address.address(&mut out, &public_key.0);
			out
		});
		(secret_key, public_key, key_pair, account)
	};

	(account1, account2)
}
