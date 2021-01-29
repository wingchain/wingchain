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
use node_consensus_base::ConsensusInMessage;
use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::codec::Decode;
use primitives::{Address, Balance};
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_poa_contract_hw_read() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, _account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_service(&authority_accounts, account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: ori_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
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

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let block_number = 1;

	let block_hash = chain.get_block_hash(&1).unwrap().unwrap();
	let header = chain.get_header(&block_hash).unwrap().unwrap();

	// hello validate
	let error = chain
		.execute_call_with_block_number::<_, Vec<u8>>(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "hello".to_string(),
				params: r#"{"name": ""}"#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap_err();
	log::info!("error: {}", error);
	assert_eq!(error, r#"ContractError: Empty name"#.to_string(),);

	// hello
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "hello".to_string(),
				params: r#"{"name": "world"}"#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#""hello world""#.to_string(),);

	// error
	let error: String = chain
		.execute_call_with_block_number::<_, Vec<u8>>(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "error".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap_err();
	log::info!("error: {}", error);
	assert_eq!(error, r#"ContractError: Custom error"#.to_string(),);

	// get env
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "get_env".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(
		result,
		format!(
			r#"{{"number":{},"timestamp":{}}}"#,
			header.number, header.timestamp
		),
	);

	// get call env
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "get_call_env".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"{"tx_hash":null}"#,);

	// get contract env
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "get_contract_env".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(
		result,
		format!(
			r#"{{"contract_address":"{}","sender_address":"{}"}}"#,
			contract_address, account1.address
		),
	);

	// hash
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "hash".to_string(),
				params: r#"{"data":[1]}"#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(
		result,
		r#""ee155ace9c40292074cb6aff8c9ccdd273c81648ff1149ef36bcea6ebb8a3e25""#,
	);

	// address
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "address".to_string(),
				params: r#"{"data":[1]}"#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#""ca5d3fa0a6887285ef6aa85cb12960a2b6706e00""#,);

	// validate address
	let error: String = chain
		.execute_call_with_block_number::<_, Vec<u8>>(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "verify_address".to_string(),
				params: r#"{"address":"aa"}"#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap_err();
	log::info!("error: {}", error);
	assert_eq!(error, r#"ContractError: InvalidAddress"#,);

	// validate address ea
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "verify_address_ea".to_string(),
				params: r#"{"address":"aa"}"#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#""false: ContractError: InvalidAddress""#,);

	// get balance
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&block_number,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "get_balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, account1.address)
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"10"#,);

	base::safe_close(chain, txpool, consensus).await;
}

#[tokio::test]
async fn test_poa_contract_hw_write() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, _account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_service(&authority_accounts, account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: ori_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
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

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: contract_address.clone(),
							pay_value: 0,
							method: "event".to_string(),
							params: r#""#.as_bytes().to_vec(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	let _tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: contract_address.clone(),
							pay_value: 0,
							method: "set_value".to_string(),
							params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	let _tx3_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: contract_address.clone(),
							pay_value: 0,
							method: "set_map".to_string(),
							params: r#"{"key":[1,2,3],"value":"abc"}"#.as_bytes().to_vec(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 3).await;

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
		.map(|event| String::from_utf8(event.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:?}", tx1_events);
	assert_eq!(
		tx1_events,
		vec![r#"{"name":"MyEvent","data":{"foo":"bar"}}"#]
	);

	// get value
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "get_value".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"{"value":"abc"}"#.to_string(),);

	// get map
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "get_map".to_string(),
				params: r#"{"key":[1,2,3]}"#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"{"value":"abc"}"#.to_string(),);

	base::safe_close(chain, txpool, consensus).await;
}

#[tokio::test]
async fn test_poa_contract_hw_transfer_success() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_service(&authority_accounts, account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: ori_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
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

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: contract_address.clone(),
							pay_value: 4,
							method: "balance_transfer".to_string(),
							params: format!(
								r#"{{"recipient":"{}", "value": 1}}"#,
								Address((account2.address).0.clone())
							)
							.as_bytes()
							.to_vec(),
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
		.map(|event| String::from_utf8(event.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:?}", tx1_events);
	assert_eq!(
		tx1_events,
		vec![
			format!(
				r#"{{"name":"Transferred","data":{{"sender":"{}","recipient":"{}","value":4}}}}"#,
				&account1.address, &contract_address
			)
			.as_str(),
			format!(
				r#"{{"name":"Transferred","data":{{"sender":"{}","recipient":"{}","value":1}}}}"#,
				&contract_address, &account2.address
			)
			.as_str(),
		]
	);

	// check balance
	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 6);

	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&contract_address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 3);

	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account2.address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 1);

	base::safe_close(chain, txpool, consensus).await;
}

#[tokio::test]
async fn test_poa_contract_hw_transfer_failed() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_service(&authority_accounts, account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: ori_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
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

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: contract_address.clone(),
							pay_value: 4,
							method: "balance_transfer".to_string(),
							params: format!(
								r#"{{"recipient":"{}", "value": 5}}"#,
								Address((account2.address).0.clone())
							)
							.as_bytes()
							.to_vec(),
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
		.map(|event| String::from_utf8(event.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:?}", tx1_events);
	assert_eq!(tx1_events, Vec::<String>::new(),);

	// check balance
	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 10);

	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&contract_address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 0);

	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account2.address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 0);

	base::safe_close(chain, txpool, consensus).await;
}

#[tokio::test]
async fn test_poa_contract_hw_transfer_partial_failed() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_service(&authority_accounts, account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: ori_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
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

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: contract_address.clone(),
							pay_value: 4,
							method: "balance_transfer_ea".to_string(),
							params: format!(
								r#"{{"recipient":"{}", "value": 5}}"#,
								Address((account2.address).0.clone())
							)
							.as_bytes()
							.to_vec(),
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
		.map(|event| String::from_utf8(event.0).unwrap())
		.collect::<Vec<_>>();
	log::info!("tx1_events: {:?}", tx1_events);
	assert_eq!(
		tx1_events,
		vec![format!(
			r#"{{"name":"Transferred","data":{{"sender":"{}","recipient":"{}","value":4}}}}"#,
			&account1.address, &contract_address
		)
		.as_str(),]
	);

	// check balance
	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 6);

	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&contract_address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 4);

	let result: Balance = chain
		.execute_call_with_block_number(
			&2,
			Some(&account2.address),
			"balance".to_string(),
			"get_balance".to_string(),
			EmptyParams,
		)
		.unwrap()
		.unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, 0);

	base::safe_close(chain, txpool, consensus).await;
}

#[tokio::test]
async fn test_poa_contract_hw_nested_contract() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let test_accounts = test_accounts(dsa, address);
	let (account1, _account2) = (&test_accounts[0], &test_accounts[1]);

	let authority_accounts = [account1];
	let (chain, txpool, consensus) = base::get_service(&authority_accounts, account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: ori_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
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

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.secret_key.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: contract_address.clone(),
							pay_value: 0,
							method: "nested_contract_execute".to_string(),
							params: "".as_bytes().to_vec(),
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
	let tx1_error = tx1_receipt.result.unwrap_err();
	log::info!("tx1_error: {:?}", tx1_error);
	assert_eq!(tx1_error, "ContractError: NestDepthExceeded".to_string());

	base::safe_close(chain, txpool, consensus).await;
}

fn get_code() -> &'static [u8] {
	let code = include_bytes!(
		"../../../vm/contract-samples/hello-world/release/contract_samples_hello_world_bg.wasm"
	);
	code
}
