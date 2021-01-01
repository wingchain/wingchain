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

use log::info;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use node_api::support::DefaultApiSupport;
use node_api::{Api, ApiConfig};
use node_chain::module;
use node_coordinator::{Keypair, LinkedHashMap, Multiaddr, PeerId, Protocol};
use primitives::codec::Encode;
use utils_test::test_accounts;

mod base;

#[tokio::test]
async fn test_api() {
	let _ = env_logger::try_init();

	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);

	let (account1, account2) = test_accounts(dsa, address);

	let account1 = (account1.0, account1.1, account1.3);
	let account2 = (account2.0, account2.1, account2.3);

	let specs = vec![
		(
			account1.clone(),
			account1.clone(),
			Keypair::generate_ed25519(),
			3509,
		),
		(
			account1.clone(),
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
	let poa0 = &services[0].2;
	let config = ApiConfig {
		rpc_addr: "0.0.0.0:3109".to_string(),
		rpc_workers: 1,
		rpc_maxconn: 100,
	};

	let support = Arc::new(DefaultApiSupport::new(chain0.clone(), txpool0.clone()));

	let _api = Api::new(config, support);

	// chain_getBlockByNumber
	let request = r#"{"jsonrpc": "2.0", "method": "chain_getBlockByNumber", "params": ["confirmed"], "id": 1}"#;
	let response = call_rpc(request).await;
	let expected = r#"{"jsonrpc":"2.0","result":{"hash":"0x958cec447c1a3a06135e91500b38554e9322c461558e8f5c881ac988e61b0d67","header":{"number":"0x0000000000000000","timestamp":"0x00000171c4eb7136","parent_hash":"0x0000000000000000000000000000000000000000000000000000000000000000","meta_txs_root":"0x02f1c52412bdfac67715f6558498a6cd920b81945b86b1ac2d547b7bc21289b4","meta_state_root":"0x338f35dc428e0fd0da8e885980e1e1e7aa39c1aeb9d992b5f2cb7c410224b925","meta_receipts_root":"0xe6c79028e5a20c619a5faa0dde88df82f378ca796a717570ef329de275ca1282","payload_txs_root":"0xcbe666e1dff8590ccfad41047bb4a6b8a682b52d1899e3f6a1c40c9eae65e363","payload_execution_gap":"0x00","payload_execution_state_root":"0x0000000000000000000000000000000000000000000000000000000000000000","payload_execution_receipts_root":"0x0000000000000000000000000000000000000000000000000000000000000000"},"body":{"meta_txs":["0x91b00eaf36abb6c89954e1ddc7933ed55373859de12bfdf24abc7c0abb327904","0x51fbd78a099eaad87565e79c617c112a7615ef7e8d41d24facdd90ebee720dda"],"payload_txs":["0x6745417d545c3e0f7d610cadfd1ee8d450a92e89fa74bb75777950a779f2aa94","0xa0faf0ea2a0c3bf69ae5c1124199c76336b36a159826e823a9fc1cd2d7b5ff55"]}},"id":1}"#;
	info!("chain_getBlockByNumber response: {}", response);
	assert_eq!(response, expected);

	// chain_sendRawTransaction
	let tx0 = chain0
		.build_transaction(
			Some((account1.0.clone(), 0, 10)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.2.clone(),
				value: 1,
			},
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
		r#"{{"jsonrpc": "2.0", "method": "chain_getTransactionInTxPool", "params": ["0x{}"], "id": 1}}"#,
		hex::encode(&tx0_hash.0)
	);
	let response = call_rpc(&request).await;
	let expected = r#"{"jsonrpc":"2.0","result":{"hash":"0x8ece9a3e63a339d854f762ff45e2b19ce110a43efe57d2499fc2c13749c1018f","witness":{"public_key":"0x8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c","signature":"0xd3b0a9ccf9d20cf4dd03b756d8236e9ace3a632f55a65f9a8539ef7f23a4ec1da05be158e75c1d8b784e669000aac5780e5d8ef904641d630ecbc0fe9e4f9b09","nonce":"0x00000000","until":"0x000000000000000a"},"call":{"module":"balance","method":"transfer","params":"0x5043346e326b6721be4a070bfb2eb49127322fa5e40100000000000000"}},"id":1}"#;
	info!("chain_getTransactionInTxPool response: {}", response);
	assert_eq!(response, expected);

	// generate block 1
	poa0.generate_block().await.unwrap();
	base::wait_block_execution(&chain0).await;

	// chain_getTransactionByHash
	let request = format!(
		r#"{{"jsonrpc": "2.0", "method": "chain_getTransactionByHash", "params": ["0x{}"], "id": 1}}"#,
		hex::encode(&tx0_hash.0)
	);
	let response = call_rpc(&request).await;
	let expected = r#"{"jsonrpc":"2.0","result":{"hash":"0x8ece9a3e63a339d854f762ff45e2b19ce110a43efe57d2499fc2c13749c1018f","witness":{"public_key":"0x8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c","signature":"0xd3b0a9ccf9d20cf4dd03b756d8236e9ace3a632f55a65f9a8539ef7f23a4ec1da05be158e75c1d8b784e669000aac5780e5d8ef904641d630ecbc0fe9e4f9b09","nonce":"0x00000000","until":"0x000000000000000a"},"call":{"module":"balance","method":"transfer","params":"0x5043346e326b6721be4a070bfb2eb49127322fa5e40100000000000000"}},"id":1}"#;
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
		hex::encode(&account1.2 .0),
	);
	let response = call_rpc(&request).await;
	let expected = r#"{"jsonrpc":"2.0","result":"0x0900000000000000","id":1}"#;
	info!("chain_executeCall response: {}", response);
	assert_eq!(response, expected);
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
