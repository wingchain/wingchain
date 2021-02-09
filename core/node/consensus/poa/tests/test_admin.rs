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

use std::sync::Arc;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use log::info;
use node_consensus_base::{ConsensusInMessage, PeerId};
use node_coordinator::{Keypair, LinkedHashMap, Multiaddr, Protocol};
use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::Address;
use tokio::time::Duration;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_poa_update_admin() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_standalone_service(&authority_accounts, account1);

	// generate block 1
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 1).await;

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let admin: module::poa::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"poa".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::poa::Admin {
			threshold: 1,
			members: vec![(account1.address.clone(), 1)],
		}
	);

	// update admin
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"poa".to_string(),
						"update_admin".to_string(),
						module::poa::UpdateAdminParams {
							admin: module::poa::Admin {
								threshold: 2,
								members: vec![
									(account1.address.clone(), 1),
									(account2.address.clone(), 1),
								],
							},
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 2
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 2).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	let admin: module::poa::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"poa".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::poa::Admin {
			threshold: 2,
			members: vec![(account1.address.clone(), 1), (account2.address.clone(), 1)],
		}
	);

	// update admin
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account2.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"poa".to_string(),
						"update_admin".to_string(),
						module::poa::UpdateAdminParams {
							admin: module::poa::Admin {
								threshold: 1,
								members: vec![(account2.address.clone(), 1)],
							},
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;

	let tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"poa".to_string(),
						"update_admin_vote".to_string(),
						module::poa::UpdateAdminVoteParams { proposal_id: 2 },
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 2).await;

	// generate block 3
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 3).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let tx2_receipt = chain.get_receipt(&tx2_hash).unwrap().unwrap();
	log::info!("tx2_result: {:x?}", tx2_receipt.result);
	let tx2_events = tx2_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx2_events: {:x?}", tx2_events);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	let admin: module::poa::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"poa".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::poa::Admin {
			threshold: 1,
			members: vec![(account2.address.clone(), 1)],
		}
	);
}

#[tokio::test]
async fn test_poa_update_authority() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa.clone(), address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];

	let specs = vec![
		(
			authority_accounts,
			account1.clone(),
			Keypair::generate_ed25519(),
			1201,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1202,
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
	let consensus0 = &services[0].2;
	let txpool0 = &services[0].1;

	// generate block 1
	consensus0
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain0, 1).await;

	let block_number = chain0.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let authority: Address = chain0
		.execute_call_with_block_number(
			&block_number,
			None,
			"poa".to_string(),
			"get_authority".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(authority, account1.address);

	let admin: module::poa::Admin = chain0
		.execute_call_with_block_number(
			&block_number,
			None,
			"poa".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::poa::Admin {
			threshold: 1,
			members: vec![(account1.address.clone(), 1)],
		}
	);

	// update admin
	let tx1_hash = base::insert_tx(
		&chain0,
		&txpool0,
		chain0
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain0
					.build_call(
						"poa".to_string(),
						"update_admin".to_string(),
						module::poa::UpdateAdminParams {
							admin: module::poa::Admin {
								threshold: 2,
								members: vec![
									(account1.address.clone(), 1),
									(account2.address.clone(), 1),
								],
							},
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool0, 1).await;

	// generate block 2
	consensus0
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain0, 2).await;

	let tx1_receipt = chain0.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let block_number = chain0.get_confirmed_number().unwrap().unwrap();
	let admin: module::poa::Admin = chain0
		.execute_call_with_block_number(
			&block_number,
			None,
			"poa".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::poa::Admin {
			threshold: 2,
			members: vec![(account1.address.clone(), 1), (account2.address.clone(), 1)],
		}
	);

	// update authority
	let tx1_hash = base::insert_tx(
		&chain0,
		&txpool0,
		chain0
			.build_transaction(
				Some((account2.secret_key.clone(), 0, 10)),
				chain0
					.build_call(
						"poa".to_string(),
						"update_authority".to_string(),
						module::poa::UpdateAuthorityParams {
							authority: account2.address.clone(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;

	let tx2_hash = base::insert_tx(
		&chain0,
		&txpool0,
		chain0
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain0
					.build_call(
						"poa".to_string(),
						"update_authority_vote".to_string(),
						module::poa::UpdateAuthorityVoteParams { proposal_id: 1 },
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool0, 2).await;

	// generate block 3
	consensus0
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain0, 3).await;

	let tx1_receipt = chain0.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_events = tx1_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:x?}", tx1_events);

	let tx2_receipt = chain0.get_receipt(&tx2_hash).unwrap().unwrap();
	log::info!("tx2_result: {:x?}", tx2_receipt.result);
	let tx2_events = tx2_receipt
		.events
		.into_iter()
		.map(|x| {
			let event = String::from_utf8(x.0).unwrap();
			event
		})
		.collect::<Vec<_>>();
	log::info!("tx2_events: {:x?}", tx2_events);

	let block_number = chain0.get_confirmed_number().unwrap().unwrap();
	let authority: Address = chain0
		.execute_call_with_block_number(
			&block_number,
			None,
			"poa".to_string(),
			"get_authority".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(authority, account2.address);

	// generate block 4
	// account2 performs generation

	let chain1 = &services[1].0;
	let consensus1 = &services[1].2;

	loop {
		{
			let number = chain1.get_execution_number().unwrap().unwrap();
			if number == 3 {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}

	consensus1
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain1, 4).await;

	let block_number = chain0.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);
}
