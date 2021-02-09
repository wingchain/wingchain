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

use std::convert::TryInto;
use std::sync::Arc;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use node_consensus_base::ConsensusInMessage;
use node_executor::module;
use primitives::{codec, Balance, Event, Proof, Receipt};
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_poa_balance() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa.clone(), address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_standalone_service(&authority_accounts, account1);

	let proof = chain
		.get_proof(&chain.get_block_hash(&0).unwrap().unwrap())
		.unwrap()
		.unwrap();
	assert_eq!(proof, Default::default());

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
	let expected_proof: Proof = {
		let proof =
			node_consensus_poa::proof::Proof::new(&block_hash, &account1.secret_key, dsa.clone())
				.unwrap();
		proof.try_into().unwrap()
	};
	assert_eq!(proof, expected_proof);

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
