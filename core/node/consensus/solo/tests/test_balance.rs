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

use tokio::time::{delay_for, Duration};

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use node_executor::module;
use primitives::{codec, Balance, Event, Receipt};
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_solo_balance() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, account2) = test_accounts(dsa, address);

	let (chain, txpool, _solo) = base::get_service(&account1.3);

	let delay_to_insert_tx = base::time_until_next(base::duration_now(), 1000) / 2;

	// after block 0
	delay_for(delay_to_insert_tx).await;

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				"balance".to_string(),
				"transfer".to_string(),
				module::balance::TransferParams {
					recipient: account2.3.clone(),
					value: 1,
				},
			)
			.unwrap(),
	)
	.await;

	// after block 1
	delay_for(Duration::from_millis(1000)).await;

	let tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 11)),
				"balance".to_string(),
				"transfer".to_string(),
				module::balance::TransferParams {
					recipient: account2.3.clone(),
					value: 2,
				},
			)
			.unwrap(),
	)
	.await;

	// after block 2
	delay_for(Duration::from_millis(1000)).await;

	let tx3_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0, 0, 12)),
				"balance".to_string(),
				"transfer".to_string(),
				module::balance::TransferParams {
					recipient: account2.3.clone(),
					value: 3,
				},
			)
			.unwrap(),
	)
	.await;

	// after block 3
	delay_for(Duration::from_millis(1000)).await;

	// check block 1
	let balance: Balance = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.3),
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
			Some(&account1.3),
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
			Some(&account1.3),
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
					sender: account1.3,
					recipient: account2.3,
					value: 3,
				},
			)
			.unwrap()],
			result: Ok(codec::encode(&()).unwrap()),
		}
	);
}
