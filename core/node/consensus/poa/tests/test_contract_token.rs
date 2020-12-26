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
async fn test_poa_contract_token_read() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let (chain, txpool, poa) = base::get_service(&account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
        &chain,
        &txpool,
        chain
            .build_transaction(
                Some((account1.0.clone(), 0, 10)),
                "contract".to_string(),
                "create".to_string(),
                module::contract::CreateParams {
                    code: ori_code.clone(),
                    init_pay_value: 0,
                    init_method: "init".to_string(),
                    init_params: r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#.as_bytes().to_vec(),
                },
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
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	// name
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "name".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#""Bitcoin""#.to_string(),);

	// symbol
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "symbol".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#""BTC""#.to_string(),);

	// decimals
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "decimals".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"8"#.to_string(),);

	// total supply
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "total_supply".to_string(),
				params: r#""#.as_bytes().to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"2100000000000000"#.to_string(),);

	// issuer balance
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&1,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, Address((account1.3).0.clone()))
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"2100000000000000"#.to_string(),);

	base::safe_close(chain, txpool, poa).await;
}

#[tokio::test]
async fn test_poa_contract_token_transfer() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, account2) = test_accounts(dsa, address);

	let (chain, txpool, poa) = base::get_service(&account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
        &chain,
        &txpool,
        chain
            .build_transaction(
                Some((account1.0.clone(), 0, 10)),
                "contract".to_string(),
                "create".to_string(),
                module::contract::CreateParams {
                    code: ori_code.clone(),
                    init_pay_value: 0,
                    init_method: "init".to_string(),
                    init_params: r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#.as_bytes().to_vec(),
                },
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
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let _tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				"contract".to_string(),
				"execute".to_string(),
				module::contract::ExecuteParams {
					contract_address: contract_address.clone(),
					pay_value: 0,
					method: "transfer".to_string(),
					params: format!(
						r#"{{"recipient":"{}","value":100000000000000}}"#,
						Address((account2.3).0.clone())
					)
					.as_bytes()
					.to_vec(),
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 2
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	// sender balance
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, Address((account1.3).0.clone()))
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"2000000000000000"#.to_string(),);

	// recipient balance
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, Address((account2.3).0.clone()))
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"100000000000000"#.to_string(),);

	base::safe_close(chain, txpool, poa).await;
}

#[tokio::test]
async fn test_poa_contract_token_transfer_from() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, account2) = test_accounts(dsa, address);

	let (chain, txpool, poa) = base::get_service(&account1);

	let ori_code = get_code().to_vec();

	let tx1_hash = base::insert_tx(
        &chain,
        &txpool,
        chain
            .build_transaction(
                Some((account1.0.clone(), 0, 10)),
                "contract".to_string(),
                "create".to_string(),
                module::contract::CreateParams {
                    code: ori_code.clone(),
                    init_pay_value: 0,
                    init_method: "init".to_string(),
                    init_params: r#"{"name":"Bitcoin","symbol":"BTC","decimals":8,"total_supply":2100000000000000}"#.as_bytes().to_vec(),
                },
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
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let _tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				"contract".to_string(),
				"execute".to_string(),
				module::contract::ExecuteParams {
					contract_address: contract_address.clone(),
					pay_value: 0,
					method: "approve".to_string(),
					params: format!(
						r#"{{"spender":"{}","value":100000000000000}}"#,
						Address((account2.3).0.clone())
					)
					.as_bytes()
					.to_vec(),
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 2
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	// check allowance after approving
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&2,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "allowance".to_string(),
				params: format!(
					r#"{{"owner":"{}","spender":"{}"}}"#,
					Address((account1.3).0.clone()),
					Address((account2.3).0.clone())
				)
				.as_bytes()
				.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"100000000000000"#.to_string(),);

	let _tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account2.0.clone(), 0, 10)),
				"contract".to_string(),
				"execute".to_string(),
				module::contract::ExecuteParams {
					contract_address: contract_address.clone(),
					pay_value: 0,
					method: "transfer_from".to_string(),
					params: format!(
						r#"{{"sender":"{}","recipient":"{}","value":100000000}}"#,
						Address((account1.3).0.clone()),
						Address((account2.3).0.clone())
					)
					.as_bytes()
					.to_vec(),
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 3
	poa.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	// allowance after transferring from
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&3,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "allowance".to_string(),
				params: format!(
					r#"{{"owner":"{}","spender":"{}"}}"#,
					Address((account1.3).0.clone()),
					Address((account2.3).0.clone())
				)
				.as_bytes()
				.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"99999900000000"#.to_string(),);

	// sender balance
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&3,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, Address((account1.3).0.clone()))
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"2099999900000000"#.to_string(),);

	// recipient balance
	let result: Vec<u8> = chain
		.execute_call_with_block_number(
			&3,
			Some(&account1.3),
			"contract".to_string(),
			"execute".to_string(),
			module::contract::ExecuteParams {
				contract_address: contract_address.clone(),
				method: "balance".to_string(),
				params: format!(r#"{{"address":"{}"}}"#, Address((account2.3).0.clone()))
					.as_bytes()
					.to_vec(),
				pay_value: 0,
			},
		)
		.unwrap()
		.unwrap();
	let result = String::from_utf8(result).unwrap();
	log::info!("result: {}", result);
	assert_eq!(result, r#"100000000"#.to_string(),);

	base::safe_close(chain, txpool, poa).await;
}

fn get_code() -> &'static [u8] {
	let code =
		include_bytes!("../../../vm/contract-samples/token/release/contract_samples_token_bg.wasm");
	code
}
