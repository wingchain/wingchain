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
use node_executor::module;
use primitives::codec::Decode;
use primitives::Address;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_poa_contract_tb_success() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let (chain, txpool, poa) = base::get_service(&account1);

	// create token contract
	let token_code = get_token_code().to_vec();

	let tx1_hash = base::insert_tx(
        &chain,
        &txpool,
        chain
            .build_transaction(
                Some((account1.0.clone(), 0, 10)),
                chain.build_call("contract".to_string(),
                                 "create".to_string(),
                                 module::contract::CreateParams {
                                     code: token_code.clone(),
                                     init_pay_value: 0,
                                     init_method: "init".to_string(),
                                     init_params: r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#.as_bytes().to_vec(),
                                 }).unwrap(),
            )
            .unwrap(),
    )
        .await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let token_contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("token_contract_address: {:x?}", token_contract_address);

	// create token bank contract
	let token_bank_code = get_token_bank_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: token_bank_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: format!(
								r#"{{"token_contract_address":"{}"}}"#,
								token_contract_address
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
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let token_bank_contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!(
		"token_bank_contract_address: {:x?}",
		token_bank_contract_address
	);

	// approve
	let _tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: token_contract_address.clone(),
							pay_value: 0,
							method: "approve".to_string(),
							params: format!(
								r#"{{"spender":"{}","value":100}}"#,
								token_bank_contract_address
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

	// generate block 3
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	// deposit
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: token_bank_contract_address.clone(),
							pay_value: 0,
							method: "deposit".to_string(),
							params: format!(r#"{{"value":90}}"#).as_bytes().to_vec(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 4
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

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
				r#"{{"name":"Transfer","data":{{"sender":"{}","recipient":"{}","value":90}}}}"#,
				account1.3, token_bank_contract_address
			),
			format!(
				r#"{{"name":"Approval","data":{{"owner":"{}","spender":"{}","value":10}}}}"#,
				account1.3, token_bank_contract_address
			)
		]
	);

	// check bank balance after depositing
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&4,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_bank_contract_address.clone(),
				method: "balance".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"90"#.to_string(),);

	// check token balance after depositing
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&4,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, account1.3)
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"2099999999999910"#.to_string(),);

	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&4,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, token_bank_contract_address)
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"90"#.to_string(),);

	// withdraw
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: token_bank_contract_address.clone(),
							pay_value: 0,
							method: "withdraw".to_string(),
							params: format!(r#"{{"value":50}}"#).as_bytes().to_vec(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 5
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

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
			r#"{{"name":"Transfer","data":{{"sender":"{}","recipient":"{}","value":50}}}}"#,
			token_bank_contract_address, account1.3,
		),]
	);

	// check bank balance after withdrawing
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&5,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_bank_contract_address.clone(),
				method: "balance".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"40"#.to_string(),);

	// check token balance after withdrawing
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&5,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, account1.3)
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"2099999999999960"#.to_string(),);

	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&5,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, token_bank_contract_address)
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"40"#.to_string(),);

	base::safe_close(chain, txpool, poa).await;
}

#[tokio::test]
async fn test_poa_contract_tb_failed() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let (chain, txpool, poa) = base::get_service(&account1);

	// create token contract
	let token_code = get_token_code().to_vec();

	let tx1_hash = base::insert_tx(
        &chain,
        &txpool,
        chain
            .build_transaction(
                Some((account1.0.clone(), 0, 10)),
                chain.build_call("contract".to_string(),
                                 "create".to_string(),
                                 module::contract::CreateParams {
                                     code: token_code.clone(),
                                     init_pay_value: 0,
                                     init_method: "init".to_string(),
                                     init_params: r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#.as_bytes().to_vec(),
                                 }).unwrap(),
            )
            .unwrap(),
    )
        .await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let token_contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("token_contract_address: {:x?}", token_contract_address);

	// create token bank contract
	let token_bank_code = get_token_bank_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: token_bank_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: format!(
								r#"{{"token_contract_address":"{}"}}"#,
								token_contract_address
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
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let token_bank_contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!(
		"token_bank_contract_address: {:x?}",
		token_bank_contract_address
	);

	// approve
	let _tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: token_contract_address.clone(),
							pay_value: 0,
							method: "approve".to_string(),
							params: format!(
								r#"{{"spender":"{}","value":100}}"#,
								token_bank_contract_address
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

	// generate block 3
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	// deposit
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: token_bank_contract_address.clone(),
							pay_value: 0,
							method: "deposit".to_string(),
							params: format!(r#"{{"value":110}}"#).as_bytes().to_vec(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 4
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_error = tx1_receipt.result.unwrap_err();
	assert_eq!(tx1_error, "ContractError: Exceed allowance".to_string());

	// check bank balance after depositing
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&4,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_bank_contract_address.clone(),
				method: "balance".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"0"#.to_string(),);

	base::safe_close(chain, txpool, poa).await;
}

#[tokio::test]
async fn test_poa_contract_tb_ea() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let (chain, txpool, poa) = base::get_service(&account1);

	// create token contract
	let token_code = get_token_code().to_vec();

	let tx1_hash = base::insert_tx(
        &chain,
        &txpool,
        chain
            .build_transaction(
                Some((account1.0.clone(), 0, 10)),
                chain.build_call("contract".to_string(),
                                 "create".to_string(),
                                 module::contract::CreateParams {
                                     code: token_code.clone(),
                                     init_pay_value: 0,
                                     init_method: "init".to_string(),
                                     init_params: r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#.as_bytes().to_vec(),
                                 }).unwrap(),
            )
            .unwrap(),
    )
        .await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let token_contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("token_contract_address: {:x?}", token_contract_address);

	// create token bank contract
	let token_bank_code = get_token_bank_code().to_vec();

	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"create".to_string(),
						module::contract::CreateParams {
							code: token_bank_code.clone(),
							init_pay_value: 0,
							init_method: "init".to_string(),
							init_params: format!(
								r#"{{"token_contract_address":"{}"}}"#,
								token_contract_address
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
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let token_bank_contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!(
		"token_bank_contract_address: {:x?}",
		token_bank_contract_address
	);

	// approve
	let _tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: token_contract_address.clone(),
							pay_value: 0,
							method: "approve".to_string(),
							params: format!(
								r#"{{"spender":"{}","value":100}}"#,
								token_bank_contract_address
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

	// generate block 3
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	// deposit
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				chain
					.build_call(
						"contract".to_string(),
						"execute".to_string(),
						module::contract::ExecuteParams {
							contract_address: token_bank_contract_address.clone(),
							pay_value: 0,
							method: "deposit_ea".to_string(),
							params: format!(r#"{{"value":110}}"#).as_bytes().to_vec(),
						},
					)
					.unwrap(),
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 4
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let tx1_result: Vec<u8> = Decode::decode(&mut &tx1_result[..]).unwrap();
	let tx1_result = String::from_utf8(tx1_result).unwrap();
	assert_eq!(tx1_result, r#""false: ContractError: Exceed allowance""#);

	// check bank balance after depositing
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&4,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: token_bank_contract_address.clone(),
				method: "balance".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"0"#.to_string(),);

	base::safe_close(chain, txpool, poa).await;
}

fn get_token_bank_code() -> &'static [u8] {
	let code = include_bytes!(
		"../../../vm/contract-samples/token-bank/release/contract_samples_token_bank_bg.wasm"
	);
	code
}

fn get_token_code() -> &'static [u8] {
	let code =
		include_bytes!("../../../vm/contract-samples/token/release/contract_samples_token_bg.wasm");
	code
}
