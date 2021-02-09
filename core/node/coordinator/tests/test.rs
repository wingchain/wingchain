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

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use log::info;
use node_consensus_base::ConsensusInMessage;
use node_executor::module;
use node_network::{Keypair, LinkedHashMap, Multiaddr, PeerId, Protocol};
use std::sync::Arc;
use tokio::time::Duration;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_coordinator_block_sync() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];

	let specs = vec![
		(
			authority_accounts,
			account1.clone(),
			Keypair::generate_ed25519(),
			1101,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1102,
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

	let services = specs
		.iter()
		.map(|x| base::get_service(&x.0, &x.1, x.2.clone(), x.3, bootnodes.clone()))
		.collect::<Vec<_>>();

	let chain0 = &services[0].0;
	let txpool0 = &services[0].1;
	let consensus0 = &services[0].2;

	// generate block 1
	consensus0
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain0, 1).await;

	// generate block 2
	let _tx1_hash = base::insert_tx(
		&chain0,
		&txpool0,
		chain0
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain0
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 1,
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool0, 1).await;

	consensus0
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain0, 2).await;

	// generate block 3
	consensus0
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain0, 3).await;

	// wait chain1 to sync
	let chain1 = &services[1].0;
	loop {
		{
			let number = chain1.get_execution_number().unwrap().unwrap();
			if number == 3 {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}

	let chain0_block_3_hash = chain0.get_block_hash(&3).unwrap().unwrap();
	let chain1_block_3_hash = chain0.get_block_hash(&3).unwrap().unwrap();

	assert_eq!(chain0_block_3_hash, chain1_block_3_hash);
}

#[tokio::test]
async fn test_coordinator_txpool_sync() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];

	let specs = vec![
		(
			authority_accounts,
			account1.clone(),
			Keypair::generate_ed25519(),
			1103,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1104,
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

	let services = specs
		.iter()
		.map(|x| base::get_service(&x.0, &x.1, x.2.clone(), x.3, bootnodes.clone()))
		.collect::<Vec<_>>();

	let chain0 = &services[0].0;
	let txpool0 = &services[0].1;

	let chain1 = &services[1].0;
	let txpool1 = &services[1].1;

	// insert tx in chain1
	let _tx1_hash = base::insert_tx(
		&chain1,
		&txpool1,
		chain1
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain0
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 1,
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool1, 1).await;

	// wait chain0 to sync
	loop {
		{
			let count = txpool0.get_map().len();
			if count == 1 {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}
}
