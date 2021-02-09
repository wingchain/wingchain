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
use derive_more::{From, TryInto};
use log::info;
use node_consensus_base::ConsensusInMessage;
use node_consensus_raft::proof::Proof;
use node_coordinator::{Keypair, LinkedHashMap, Multiaddr, PeerId, Protocol};
use node_executor::module;
use primitives::codec::{Decode, Encode};
use primitives::{codec, Balance, Event, Receipt};
use std::convert::TryInto;
use std::sync::Arc;
use utils_enum_codec::enum_codec;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_raft_balance_3_authorities() {
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
			1301,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1302,
		),
		(
			authority_accounts,
			account3.clone(),
			Keypair::generate_ed25519(),
			1303,
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

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
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
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 1).await;

	let block_hash = chain.get_block_hash(&1).unwrap().unwrap();
	let proof = chain.get_proof(&block_hash).unwrap().unwrap();
	let _proof: Proof = Decode::decode(&mut &proof.data[..]).unwrap();

	let tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 11)),
				chain
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 2,
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

	let tx3_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 12)),
				chain
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 3,
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 3
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 3).await;

	// check block 1
	let balance: Balance = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 9);
	let block1 = chain
		.get_block(&chain.get_block_hash(&1).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block1.body.payload_txs[0], tx1_hash);

	// check block 2
	let balance: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 7);

	let block2 = chain
		.get_block(&chain.get_block_hash(&2).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block2.body.payload_txs[0], tx2_hash);

	// check block 3
	let balance: Balance = chain
		.execute_call_with_block_number(
			&3,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 4);

	let block3 = chain
		.get_block(&chain.get_block_hash(&3).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block3.body.payload_txs[0], tx3_hash);

	let tx3_receipt = chain.get_receipt(&tx3_hash).unwrap().unwrap();
	assert_eq!(
		tx3_receipt,
		Receipt {
			block_number: 3,
			events: vec![Event::from_data(
				"Transferred".to_string(),
				module::balance::Transferred {
					sender: account1.address.clone(),
					recipient: account2.address.clone(),
					value: 3,
				},
			)
			.unwrap()],
			result: Ok(codec::encode(&()).unwrap()),
		}
	);
}

#[tokio::test]
async fn test_raft_balance_2_authorities() {
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
			1304,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			1305,
		),
		(
			authority_accounts,
			account3.clone(),
			Keypair::generate_ed25519(),
			1306,
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

	let mut services = specs
		.iter()
		.map(|x| base::get_service(&x.0, &x.1, x.2.clone(), x.3, bootnodes.clone()))
		.collect::<Vec<_>>();

	drop(services.remove(2));

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

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
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
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 1).await;

	let block_hash = chain.get_block_hash(&1).unwrap().unwrap();
	let proof = chain.get_proof(&block_hash).unwrap().unwrap();
	let _proof: Proof = Decode::decode(&mut &proof.data[..]).unwrap();

	let tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 11)),
				chain
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 2,
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

	let tx3_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 12)),
				chain
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 3,
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 3
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 3).await;

	// check block 1
	let balance: Balance = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 9);
	let block1 = chain
		.get_block(&chain.get_block_hash(&1).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block1.body.payload_txs[0], tx1_hash);

	// check block 2
	let balance: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 7);

	let block2 = chain
		.get_block(&chain.get_block_hash(&2).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block2.body.payload_txs[0], tx2_hash);

	// check block 3
	let balance: Balance = chain
		.execute_call_with_block_number(
			&3,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 4);

	let block3 = chain
		.get_block(&chain.get_block_hash(&3).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block3.body.payload_txs[0], tx3_hash);

	let tx3_receipt = chain.get_receipt(&tx3_hash).unwrap().unwrap();
	assert_eq!(
		tx3_receipt,
		Receipt {
			block_number: 3,
			events: vec![Event::from_data(
				"Transferred".to_string(),
				module::balance::Transferred {
					sender: account1.address.clone(),
					recipient: account2.address.clone(),
					value: 3,
				},
			)
			.unwrap()],
			result: Ok(codec::encode(&()).unwrap()),
		}
	);
}

#[tokio::test]
async fn test_raft_balance_1_authority() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa.clone(), address);
	let (account1, account2, _account3) = (&test_accounts[0], &test_accounts[1], &test_accounts[2]);

	let authority_accounts = [account1];

	let specs = vec![(
		authority_accounts,
		account1.clone(),
		Keypair::generate_ed25519(),
		1307,
	)];

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

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
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
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 1).await;

	let block_hash = chain.get_block_hash(&1).unwrap().unwrap();
	let proof = chain.get_proof(&block_hash).unwrap().unwrap();
	let _proof: Proof = Decode::decode(&mut &proof.data[..]).unwrap();

	let tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 11)),
				chain
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 2,
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

	let tx3_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 12)),
				chain
					.build_call(
						"balance".to_string(),
						"transfer".to_string(),
						module::balance::TransferParams {
							recipient: account2.address.clone(),
							value: 3,
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 3
	consensus
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain, 3).await;

	// check block 1
	let balance: Balance = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 9);
	let block1 = chain
		.get_block(&chain.get_block_hash(&1).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block1.body.payload_txs[0], tx1_hash);

	// check block 2
	let balance: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 7);

	let block2 = chain
		.get_block(&chain.get_block_hash(&2).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block2.body.payload_txs[0], tx2_hash);

	// check block 3
	let balance: Balance = chain
		.execute_call_with_block_number(
			&3,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			node_executor_primitives::EmptyParams,
		)
		.unwrap()
		.unwrap();
	assert_eq!(balance, 4);

	let block3 = chain
		.get_block(&chain.get_block_hash(&3).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(block3.body.payload_txs[0], tx3_hash);

	let tx3_receipt = chain.get_receipt(&tx3_hash).unwrap().unwrap();
	assert_eq!(
		tx3_receipt,
		Receipt {
			block_number: 3,
			events: vec![Event::from_data(
				"Transferred".to_string(),
				module::balance::Transferred {
					sender: account1.address.clone(),
					recipient: account2.address.clone(),
					value: 3,
				},
			)
			.unwrap()],
			result: Ok(codec::encode(&()).unwrap()),
		}
	);
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
