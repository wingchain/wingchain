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
use node_executor::module::raft::Authorities;
use node_executor_primitives::EmptyParams;
use tokio::time::Duration;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_raft_update_admin() {
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
			1308,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1309,
		),
		(
			authority_accounts,
			account3.clone(),
			Keypair::generate_ed25519(),
			1310,
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
		info!("address: {}", spec.1.address);
		info!("peer id: {}", spec.2.public().into_peer_id());
	}

	let services = specs
		.iter()
		.map(|x| base::get_service(&x.0, &x.1, x.2.clone(), x.3, bootnodes.clone()))
		.collect::<Vec<_>>();

	let consensus0 = &services[0].2;

	let leader_address = base::wait_leader_elected(&consensus0).await;

	let leader_index = specs
		.iter()
		.position(|x| x.1.address == leader_address)
		.unwrap();

	let leader_service = &services[leader_index];

	let chain = &leader_service.0;
	let txpool = &leader_service.1;
	let consensus = &leader_service.2;

	// generate block 1
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 1).await;

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let admin: module::raft::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::raft::Admin {
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
						"raft".to_string(),
						"update_admin".to_string(),
						module::raft::UpdateAdminParams {
							admin: module::raft::Admin {
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
	let admin: module::raft::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::raft::Admin {
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
						"raft".to_string(),
						"update_admin".to_string(),
						module::raft::UpdateAdminParams {
							admin: module::raft::Admin {
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
						"raft".to_string(),
						"update_admin_vote".to_string(),
						module::raft::UpdateAdminVoteParams { proposal_id: 2 },
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
	let admin: module::raft::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::raft::Admin {
			threshold: 1,
			members: vec![(account2.address.clone(), 1)],
		}
	);
}

#[tokio::test]
async fn test_raft_update_authorities_drop_follower() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa.clone(), address);
	let (account1, account2, account3, account4, account5) = (
		&test_accounts[0],
		&test_accounts[1],
		&test_accounts[2],
		&test_accounts[3],
		&test_accounts[4],
	);

	let authority_accounts = [account1, account2, account3];

	let specs = vec![
		(
			authority_accounts,
			account1.clone(),
			Keypair::generate_ed25519(),
			1311,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1312,
		),
		(
			authority_accounts,
			account3.clone(),
			Keypair::generate_ed25519(),
			1313,
		),
		(
			authority_accounts,
			account4.clone(),
			Keypair::generate_ed25519(),
			1314,
		),
		(
			authority_accounts,
			account5.clone(),
			Keypair::generate_ed25519(),
			1315,
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

	let consensus0 = &services[0].2;

	let leader_address = base::wait_leader_elected(&consensus0).await;

	let leader_index = specs
		.iter()
		.position(|x| x.1.address == leader_address)
		.unwrap();

	info!("leader_index: {}", leader_index);

	let leader_service = &services[leader_index];

	let chain = &leader_service.0;
	let txpool = &leader_service.1;
	let consensus = &leader_service.2;

	// generate block 1
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 1).await;

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let authorities: Authorities = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_authorities".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(
		authorities,
		Authorities {
			members: vec![
				account1.address.clone(),
				account2.address.clone(),
				account3.address.clone(),
			]
		}
	);

	let admin: module::raft::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::raft::Admin {
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
						"raft".to_string(),
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
			"raft".to_string(),
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

	let new_authorities = vec![
		specs[leader_index].1.address.clone(),
		account4.address.clone(),
		account5.address.clone(),
	];

	// update authorities
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account2.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"raft".to_string(),
						"update_authorities".to_string(),
						module::raft::UpdateAuthoritiesParams {
							authorities: Authorities {
								members: new_authorities.clone(),
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
						"raft".to_string(),
						"update_authorities_vote".to_string(),
						module::raft::UpdateAuthoritiesVoteParams { proposal_id: 1 },
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
	let authorities: Authorities = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_authorities".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(
		authorities,
		Authorities {
			members: new_authorities,
		}
	);

	let leader_address = base::wait_leader_elected(&consensus0).await;

	let leader_index = specs
		.iter()
		.position(|x| x.1.address == leader_address)
		.unwrap();

	info!("leader_index: {}", leader_index);

	let leader_service = &services[leader_index];

	let chain = &leader_service.0;
	let _txpool = &leader_service.1;
	let consensus = &leader_service.2;

	// generate block 4

	loop {
		{
			let number = chain.get_execution_number().unwrap().unwrap();
			if number == 3 {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}

	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 4).await;

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);
}

#[tokio::test]
async fn test_raft_update_authorities_drop_leader() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa.clone(), address);
	let (account1, account2, account3, account4, account5) = (
		&test_accounts[0],
		&test_accounts[1],
		&test_accounts[2],
		&test_accounts[3],
		&test_accounts[4],
	);

	let authority_accounts = [account1, account2, account3];

	let specs = vec![
		(
			authority_accounts,
			account1.clone(),
			Keypair::generate_ed25519(),
			1316,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1317,
		),
		(
			authority_accounts,
			account3.clone(),
			Keypair::generate_ed25519(),
			1318,
		),
		(
			authority_accounts,
			account4.clone(),
			Keypair::generate_ed25519(),
			1319,
		),
		(
			authority_accounts,
			account5.clone(),
			Keypair::generate_ed25519(),
			1320,
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

	let consensus0 = &services[0].2;

	let leader_address = base::wait_leader_elected(&consensus0).await;

	let leader_index = specs
		.iter()
		.position(|x| x.1.address == leader_address)
		.unwrap();

	info!("leader_index: {}", leader_index);

	let leader_service = &services[leader_index];

	let chain = &leader_service.0;
	let txpool = &leader_service.1;
	let consensus = &leader_service.2;

	// generate block 1
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 1).await;

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let authorities: Authorities = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_authorities".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(
		authorities,
		Authorities {
			members: vec![
				account1.address.clone(),
				account2.address.clone(),
				account3.address.clone(),
			]
		}
	);

	let admin: module::raft::Admin = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_admin".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		module::raft::Admin {
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
						"raft".to_string(),
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
			"raft".to_string(),
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

	let reserved_index = (leader_index + 1) % 3;
	let new_authorities = vec![
		specs[reserved_index].1.address.clone(),
		account4.address.clone(),
		account5.address.clone(),
	];

	// update authorities
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account2.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"raft".to_string(),
						"update_authorities".to_string(),
						module::raft::UpdateAuthoritiesParams {
							authorities: Authorities {
								members: new_authorities.clone(),
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
						"raft".to_string(),
						"update_authorities_vote".to_string(),
						module::raft::UpdateAuthoritiesVoteParams { proposal_id: 1 },
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
	let authorities: Authorities = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"raft".to_string(),
			"get_authorities".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(
		authorities,
		Authorities {
			members: new_authorities,
		}
	);

	let reserved_consensus = &services[reserved_index].2;

	let leader_address = loop {
		let new_leader_address = base::wait_leader_elected(&reserved_consensus).await;
		if new_leader_address != leader_address {
			break new_leader_address;
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	};

	let leader_index = specs
		.iter()
		.position(|x| x.1.address == leader_address)
		.unwrap();

	info!("leader_index: {}", leader_index);

	let leader_service = &services[leader_index];

	let chain = &leader_service.0;
	let _txpool = &leader_service.1;
	let consensus = &leader_service.2;

	// generate block 4

	loop {
		{
			let number = chain.get_execution_number().unwrap().unwrap();
			if number == 3 {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}

	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 4).await;

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);
}
