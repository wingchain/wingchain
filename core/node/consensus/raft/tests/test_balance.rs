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
//! TODO rm
#![allow(dead_code)]

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use derive_more::{From, TryInto};
use log::info;
use node_coordinator::{Keypair, LinkedHashMap, Multiaddr, PeerId, Protocol};
use primitives::codec::{Decode, Encode};
use std::convert::TryInto;
use std::sync::Arc;
use tokio::time::Duration;
use utils_enum_codec::enum_codec;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_raft_balance() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa.clone(), address);
	let (account1, account2, account3) = (&test_accounts[0], &test_accounts[1], &test_accounts[2]);

	let authority_accounts = [account1, account2, account3];

	let specs = vec![
		(
			authority_accounts,
			account1.clone(),
			Keypair::generate_ed25519(),
			3609,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			3610,
		),
		(
			authority_accounts,
			account3.clone(),
			Keypair::generate_ed25519(),
			3611,
		),
	];

	let bootnodes = {
		let bootnodes_spec = &specs[0];
		let bootnodes = (
			bootnodes_spec.2.public().into_peer_id(),
			Multiaddr::empty()
				.with(Protocol::Ip4([127, 0, 0, 1].into()))
				.with(Protocol::Tcp(bootnodes_spec.3)),
		);
		let bootnodes =
			std::iter::once((bootnodes, ())).collect::<LinkedHashMap<(PeerId, Multiaddr), ()>>();
		bootnodes
	};

	for spec in &specs {
		info!("peer id: {}", spec.2.public().into_peer_id());
	}

	let _services = specs
		.iter()
		.map(|x| base::get_service(&x.0, &x.1, x.2.clone(), x.3, bootnodes.clone()))
		.collect::<Vec<_>>();

	tokio::time::sleep(Duration::from_secs(10)).await;
}

#[test]
fn test_enum_codec() {
	#[derive(Encode, Decode, Debug, PartialEq)]
	struct A {
		a: String,
	}

	#[derive(Encode, Decode, Debug, PartialEq)]
	struct B {
		b: u32,
	}

	#[enum_codec]
	#[derive(TryInto, From, Debug, PartialEq)]
	enum E {
		A(A),
		B(B),
	}

	let a: E = A {
		a: "test".to_string(),
	}
	.into();
	let a = a.encode();
	assert_eq!(a, vec![4, 65, 16, 116, 101, 115, 116]);
	let a: E = Decode::decode(&mut &a[..]).unwrap();
	let a: A = a.try_into().unwrap();
	assert_eq!(
		a,
		A {
			a: "test".to_string()
		}
	);

	let b: E = B { b: 10 }.into();
	let b = b.encode();
	assert_eq!(b, vec![4, 66, 10, 0, 0, 0]);
	let b: E = Decode::decode(&mut &b[..]).unwrap();
	let b: B = b.try_into().unwrap();
	assert_eq!(b, B { b: 10 });
}
