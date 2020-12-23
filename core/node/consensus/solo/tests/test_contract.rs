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
use crypto::hash::{Hash as HashT, HashImpl};
use node_executor::module;
use primitives::codec::Decode;
use primitives::{Address, Hash};
use utils_test::test_accounts;

mod base;

#[async_std::test]
async fn test_solo_contract_create() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let (chain, txpool, solo) = base::get_service(&account1.3);

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
					init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	solo.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);
	let version: Option<u32> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_version".to_string(),
			module::contract::GetVersionParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("version: {}", version.unwrap());
	assert_eq!(version, Some(1));

	let code: Option<Vec<u8>> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code".to_string(),
			module::contract::GetCodeParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	assert_eq!(code, Some(ori_code));

	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account1.3.clone(), 1)],
		})
	);
	base::safe_close(chain, txpool, solo).await;
}

#[async_std::test]
async fn test_solo_contract_create_fail() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, _account2) = test_accounts(dsa, address);

	let (chain, txpool, solo) = base::get_service(&account1.3);

	let ori_code = get_code().to_vec();

	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"create".to_string(),
			module::contract::CreateParams {
				code: ori_code.clone(),
				init_pay_value: 0,
				init_method: "init".to_string(),
				init_params: r#"{"value1":"abc"}"#.as_bytes().to_vec(),
			},
		)
		.unwrap();

	let tx1_error = chain.validate_transaction(&tx1, true).unwrap_err();
	assert_eq!(
		tx1_error.to_string(),
		"[CommonError] Kind: Executor, Error: ContractError: InvalidParams".to_string()
	);

	let tx1 = chain
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"contract".to_string(),
			"create".to_string(),
			module::contract::CreateParams {
				code: vec![1; 1024],
				init_pay_value: 0,
				init_method: "init".to_string(),
				init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
			},
		)
		.unwrap();
	let tx1_error = chain.validate_transaction(&tx1, true).unwrap_err();

	assert_eq!(tx1_error.to_string(), "[CommonError] Kind: Executor, Error: PreCompileError: ValidationError: Bad magic number (at offset 0)".to_string());

	base::safe_close(chain, txpool, solo).await;
}

#[async_std::test]
async fn test_solo_contract_update_admin() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, account2) = test_accounts(dsa, address);

	let (chain, txpool, solo) = base::get_service(&account1.3);

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
					init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	solo.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account1.3.clone(), 1)],
		})
	);

	// update admin
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				"contract".to_string(),
				"update_admin".to_string(),
				module::contract::UpdateAdminParams {
					contract_address: contract_address.clone(),
					admin: module::contract::Admin {
						threshold: 2,
						members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
					},
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 2
	solo.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

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
	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 2,
			members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
		})
	);

	// update admin
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account2.0.clone(), 0, 10)),
				"contract".to_string(),
				"update_admin".to_string(),
				module::contract::UpdateAdminParams {
					contract_address: contract_address.clone(),
					admin: module::contract::Admin {
						threshold: 1,
						members: vec![(account2.3.clone(), 1)],
					},
				},
			)
			.unwrap(),
	)
	.await;

	let tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				"contract".to_string(),
				"update_admin_vote".to_string(),
				module::contract::UpdateAdminVoteParams {
					contract_address: contract_address.clone(),
					proposal_id: 2,
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 2).await;

	// generate block 3
	solo.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

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
	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account2.3.clone(), 1)],
		})
	);

	base::safe_close(chain, txpool, solo).await;
}

#[async_std::test]
async fn test_solo_contract_update_code() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);
	let hasher = Arc::new(HashImpl::Blake2b256);

	let (account1, account2) = test_accounts(dsa, address);

	let (chain, txpool, solo) = base::get_service(&account1.3);

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
					init_params: r#"{"value":"abc"}"#.as_bytes().to_vec(),
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 1
	solo.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

	let tx1_receipt = chain.get_receipt(&tx1_hash).unwrap().unwrap();
	let tx1_result = tx1_receipt.result.unwrap();
	let contract_address: Address = Decode::decode(&mut &tx1_result[..]).unwrap();
	log::info!("contract_address: {:x?}", contract_address);

	let block_number = chain.get_confirmed_number().unwrap().unwrap();
	log::info!("block_number: {}", block_number);

	let code: Option<Vec<u8>> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code".to_string(),
			module::contract::GetCodeParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	assert_eq!(code, Some(ori_code));

	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 1,
			members: vec![(account1.3.clone(), 1)],
		})
	);

	// update admin
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				"contract".to_string(),
				"update_admin".to_string(),
				module::contract::UpdateAdminParams {
					contract_address: contract_address.clone(),
					admin: module::contract::Admin {
						threshold: 2,
						members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
					},
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 1).await;

	// generate block 2
	solo.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

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
	let admin: Option<module::contract::Admin> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_admin".to_string(),
			module::contract::GetAdminParams {
				contract_address: contract_address.clone(),
			},
		)
		.unwrap()
		.unwrap();
	log::info!("admin: {:?}", admin);
	assert_eq!(
		admin,
		Some(module::contract::Admin {
			threshold: 2,
			members: vec![(account1.3.clone(), 1), (account2.3.clone(), 1)],
		})
	);

	// update code
	let new_code = get_code().to_vec();
	let tx1_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account2.0.clone(), 0, 10)),
				"contract".to_string(),
				"update_code".to_string(),
				module::contract::UpdateCodeParams {
					contract_address: contract_address.clone(),
					code: new_code.clone(),
				},
			)
			.unwrap(),
	)
	.await;

	let tx2_hash = base::insert_tx(
		&chain,
		&txpool,
		chain
			.build_transaction(
				Some((account1.0.clone(), 0, 10)),
				"contract".to_string(),
				"update_code_vote".to_string(),
				module::contract::UpdateCodeVoteParams {
					contract_address: contract_address.clone(),
					proposal_id: 1,
				},
			)
			.unwrap(),
	)
	.await;
	base::wait_txpool(&txpool, 2).await;

	// generate block 3
	solo.generate_block().await.unwrap();
	base::wait_block_execution(&chain).await;

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
	let code: Option<Vec<u8>> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code".to_string(),
			module::contract::GetCodeParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	assert_eq!(code, Some(new_code.clone()),);

	let code_hash: Option<Hash> = chain
		.execute_call_with_block_number(
			&block_number,
			None,
			"contract".to_string(),
			"get_code_hash".to_string(),
			module::contract::GetCodeHashParams {
				contract_address: contract_address.clone(),
				version: None,
			},
		)
		.unwrap()
		.unwrap();
	let expect_code_hash = {
		let mut out = vec![0; hasher.length().into()];
		hasher.hash(&mut out, &new_code);
		Hash(out)
	};
	log::info!("code_hash: {:?}", code_hash);
	assert_eq!(code_hash, Some(expect_code_hash),);

	base::safe_close(chain, txpool, solo).await;
}

fn get_code() -> &'static [u8] {
	let code = include_bytes!(
		"../../../vm/contract-samples/hello-world/release/contract_samples_hello_world_bg.wasm"
	);
	code
}
