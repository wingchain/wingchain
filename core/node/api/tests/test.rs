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

use log::info;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use node_api::support::DefaultApiSupport;
use node_api::{Api, ApiConfig};
use node_api_rt::tokio::time::Duration;
use node_chain::module;
use node_consensus_base::ConsensusInMessage;
use node_coordinator::{Keypair, LinkedHashMap, Multiaddr, PeerId, Protocol};
use primitives::codec::Encode;
use primitives::Proof;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_api() {
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
			3509,
		),
		(
			authority_accounts,
			account2.clone(),
			Keypair::generate_ed25519(),
			3510,
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
	let coordinator0 = &services[0].3;
	let config = ApiConfig {
		rpc_addr: "0.0.0.0:3109".to_string(),
		rpc_workers: 1,
		rpc_maxconn: 100,
	};

	let support = Arc::new(DefaultApiSupport::new(
		chain0.clone(),
		txpool0.clone(),
		coordinator0.clone(),
	));

	let _api = Api::new(config, support);

	// chain_getBlockByNumber
	let request = r#"{"jsonrpc": "2.0", "method": "chain_getBlockByNumber", "params": ["confirmed"], "id": 1}"#;
	let response = call_rpc(request).await;
	let expected = r#"{"jsonrpc":"2.0","result":{"hash":"0x5d9c463e3666a5d9f51f2e674f1f2f7eebbc0693d30ba0cd4c34e46b2be75a11","header":{"number":"0x0000000000000000","timestamp":"0x00000171c4eb7136","parent_hash":"0x0000000000000000000000000000000000000000000000000000000000000000","meta_txs_root":"0x3eb8cb0c85f0ecc98300e23d753bc48a1ab5e42f86c9bba75a5c914f269d81ad","meta_state_root":"0xaebe53e5c921912056f086387df448d734ded82eac8479dea73c2ddf8f39274a","meta_receipts_root":"0xe6c79028e5a20c619a5faa0dde88df82f378ca796a717570ef329de275ca1282","payload_txs_root":"0xcbe666e1dff8590ccfad41047bb4a6b8a682b52d1899e3f6a1c40c9eae65e363","payload_execution_gap":"0x00","payload_execution_state_root":"0x0000000000000000000000000000000000000000000000000000000000000000","payload_execution_receipts_root":"0x0000000000000000000000000000000000000000000000000000000000000000"},"body":{"meta_txs":["0x709ab477fc45b28aab319323399bee607bac3af49518e33a78f099c6916ef75e","0x51fbd78a099eaad87565e79c617c112a7615ef7e8d41d24facdd90ebee720dda"],"payload_txs":["0x6745417d545c3e0f7d610cadfd1ee8d450a92e89fa74bb75777950a779f2aa94","0xa0faf0ea2a0c3bf69ae5c1124199c76336b36a159826e823a9fc1cd2d7b5ff55"]}},"id":1}"#;
	info!("chain_getBlockByNumber response: {}", response);
	assert_eq!(response, expected);

	// chain_sendRawTransaction
	let tx0 = chain0
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
		.unwrap();
	let tx0_hash = chain0.hash_transaction(&tx0).unwrap();
	let tx0_raw = hex::encode(tx0.encode());

	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "chain_sendRawTransaction", "params": ["0x{}"], "id": 1}}"#,
		tx0_raw
	);
	let response = call_rpc(&request).await;
	let expected = format!(
		r#"{{"jsonrpc":"2.0","result":"0x{}","id":1}}"#,
		hex::encode(&tx0_hash.0)
	);
	info!("chain_sendRawTransaction response: {}", response);
	assert_eq!(response, expected);

	// chain_getTransactionInTxPool
	base::wait_txpool(&txpool0, 1).await;

	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "txpool_getTransaction", "params": ["0x{}"], "id": 1}}"#,
		hex::encode(&tx0_hash.0)
	);
	let response = call_rpc(&request).await;
	let expected = r#"{"jsonrpc":"2.0","result":{"hash":"0x8ece9a3e63a339d854f762ff45e2b19ce110a43efe57d2499fc2c13749c1018f","witness":{"public_key":"0x8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c","signature":"0x4b84ec33e868a6875bb1297b2c06e7598546f80addd9f83ba36e9d96af30e2d9e652bb37be486e59f0c60e346cca7d7ca4385295c0ecb9a78a4d7d217dd1cb0b","nonce":"0x00000000","until":"0x000000000000000a"},"call":{"module":"balance","method":"transfer","params":"0x5043346e326b6721be4a070bfb2eb49127322fa5e40100000000000000"}},"id":1}"#;
	info!("chain_getTransactionInTxPool response: {}", response);
	assert_eq!(response, expected);

	// generate block 1
	consensus0
		.in_message_tx()
		.unbounded_send(ConsensusInMessage::Generate)
		.unwrap();
	base::wait_block_execution(&chain0, 1).await;

	// chain_getTransactionByHash
	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "chain_getTransactionByHash", "params": ["0x{}"], "id": 1}}"#,
		hex::encode(&tx0_hash.0)
	);
	let response = call_rpc(&request).await;
	let expected = r#"{"jsonrpc":"2.0","result":{"hash":"0x8ece9a3e63a339d854f762ff45e2b19ce110a43efe57d2499fc2c13749c1018f","witness":{"public_key":"0x8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c","signature":"0x4b84ec33e868a6875bb1297b2c06e7598546f80addd9f83ba36e9d96af30e2d9e652bb37be486e59f0c60e346cca7d7ca4385295c0ecb9a78a4d7d217dd1cb0b","nonce":"0x00000000","until":"0x000000000000000a"},"call":{"module":"balance","method":"transfer","params":"0x5043346e326b6721be4a070bfb2eb49127322fa5e40100000000000000"}},"id":1}"#;
	info!("chain_getTransactionByHash response: {}", response);
	assert_eq!(response, expected);

	// chain_getRawTransactionByHash
	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "chain_getRawTransactionByHash", "params": ["0x{}"], "id": 1}}"#,
		hex::encode(&tx0_hash.0)
	);
	let response = call_rpc(&request).await;
	let expected = format!(r#"{{"jsonrpc":"2.0","result":"0x{}","id":1}}"#, tx0_raw);
	info!("chain_getRawTransactionByHash response: {}", response);
	assert_eq!(response, expected);

	// chain_getReceiptByHash
	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "chain_getReceiptByHash", "params": ["0x{}"], "id": 1}}"#,
		hex::encode(&tx0_hash.0)
	);
	let response = call_rpc(&request).await;
	let expected = r#"{"jsonrpc":"2.0","result":{"hash":"0x8ece9a3e63a339d854f762ff45e2b19ce110a43efe57d2499fc2c13749c1018f","block_number":"0x0000000000000001","events":[{"data":{"recipient":"43346e326b6721be4a070bfb2eb49127322fa5e4","sender":"b4decd5a5f8f2ba708f8ced72eec89f44f3be96a","value":1},"name":"Transferred"}],"result":{"Ok":"0x"}},"id":1}"#;
	info!("chain_getReceiptByHash response: {}", response);
	assert_eq!(response, expected);

	// chain_executeCall
	let block1_hash = chain0.get_block_hash(&1).unwrap().unwrap();
	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "chain_executeCall", "params": {{ "block_hash": "0x{}", "sender": "0x{}", "call": {{ "module":"balance", "method":"get_balance", "params": "" }} }}, "id": 1}}"#,
		hex::encode(&block1_hash.0),
		hex::encode(&account1.address.0),
	);
	let response = call_rpc(&request).await;
	let expected = r#"{"jsonrpc":"2.0","result":"0x0900000000000000","id":1}"#;
	info!("chain_executeCall response: {}", response);
	assert_eq!(response, expected);

	// chain_getProofByHash
	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "chain_getProofByHash", "params": ["0x{}"], "id": 1}}"#,
		hex::encode(&block1_hash.0)
	);
	let response = call_rpc(&request).await;
	let expected = {
		let proof =
			node_consensus_poa::proof::Proof::new(&block1_hash, &account1.secret_key, dsa.clone())
				.unwrap();
		let proof: Proof = proof.try_into().unwrap();
		format!(
			r#"{{"jsonrpc":"2.0","result":{{"hash":"0x{}","name":"{}","data":"0x{}"}},"id":1}}"#,
			hex::encode(&block1_hash.0),
			proof.name,
			hex::encode(&proof.data)
		)
	};
	info!("chain_getProofByHash response: {}", response);
	assert_eq!(response, expected);

	// wait chain1 to sync
	let chain1 = &services[1].0;
	loop {
		{
			let number = chain1.get_execution_number().unwrap().unwrap();
			if number == 1 {
				break;
			}
		}
		futures_timer::Delay::new(Duration::from_millis(10)).await;
	}

	// network_getState
	let request =
		format!(r#"{{"jsonrpc": "2.0", "method": "network_getState", "params": [], "id": 1}}"#);
	let response = call_rpc(&request).await;
	info!("network_getState response: {}", response);
	let response: serde_json::Value = serde_json::from_str(&response).unwrap();
	let opened_peers = &response["result"]["opened_peers"];
	let opened_peer_count = opened_peers.as_array().unwrap().len();
	assert_eq!(opened_peer_count, 1);
}

async fn call_rpc(request: &str) -> String {
	let mut res = surf::post("http://127.0.0.1:3109")
		.body(request)
		.send()
		.await
		.unwrap();
	let response = res.body_string().await.unwrap();
	response
}
