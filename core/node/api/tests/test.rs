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

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::tempdir;

use node_api::support::DefaultApiSupport;
use node_api::{Api, ApiConfig};
use node_chain::{Chain, ChainConfig};
use node_txpool::{TxPool, TxPoolConfig};

#[tokio::test]
async fn test_api() {
	let config = ApiConfig {
		rpc_addr: "0.0.0.0:3109".to_string(),
		rpc_workers: 1,
		rpc_maxconn: 100,
	};
	let support = Arc::new(get_support());

	let _api = Api::new(config, support);

	let client = reqwest::Client::new();

	for (request, expected_response) in get_cases() {
		let res = client
			.post("http://127.0.0.1:3109")
			.body(request)
			.send()
			.await
			.unwrap();
		let response = res.text().await.unwrap();
		assert_eq!(response, expected_response);
	}
}

fn get_cases() -> Vec<(String, String)> {
	vec![
		(
			r#"{"jsonrpc": "2.0", "method": "chain_getBlockByNumber", "params": ["best"], "id": 1}"#
				.to_string(),
			r#"{"jsonrpc":"2.0","result":{"hash":"0x210d67b3539a8bf7466e1c1dfd30088143df6efb","header":{"number":"0x00000000","timestamp":"0x5e987dba","parent_hash":"0x0000000000000000000000000000000000000000","meta_txs_root":"0x6c6fdfd66f23cd420ce336d66446cac4af1a4f2f","meta_state_root":"0x9abf22924c884d089c9b90c48b90fde40ea89867","payload_txs_root":"0x082ad992fb76871c33a1b9993a082952feaca5e6","payload_executed_gap":"0x01","payload_executed_state_root":"0x0000000000000000000000000000000000000000"},"body":{"meta_txs":["0x6f83855c8abfeff14ad9fb01f68922f4125071f8"],"payload_txs":[]}},"id":1}"#.to_string(),
		),
		(
			r#"{"jsonrpc": "2.0", "method": "chain_getTransactionByHash", "params": ["0x6f83855c8abfeff14ad9fb01f68922f4125071f8"], "id": 1}"#
				.to_string(),
			r#"{"jsonrpc":"2.0","result":{"hash":"0x6f83855c8abfeff14ad9fb01f68922f4125071f8","witness":null,"call":{"module":"system","method":"init","params":"0x0a00000000000000636861696e2d74657374ba7d985e"}},"id":1}"#.to_string(),
		),
		(
			r#"{"jsonrpc": "2.0", "method": "chain_getRawTransactionByHash", "params": ["0x6f83855c8abfeff14ad9fb01f68922f4125071f8"], "id": 1}"#.to_string(),
			r#"{"jsonrpc":"2.0","result":"0x00060000000000000073797374656d0400000000000000696e697416000000000000000a00000000000000636861696e2d74657374ba7d985e","id":1}"#.to_string(),
		),
		// (
		// 	r#"{"jsonrpc": "2.0", "method": "chain_sendRawTransaction", "params": ["0x00060000000000000073797374656d0400000000000000696e69741a000000000000000e00000000000000636861696e2d6a64726a71666868a0f79e5e"], "id": 1}"#.to_string(),
		// 	r#"{"jsonrpc":"2.0","result":"0x3b624b93cb726681ddb8d79378783eb2b3147804","id":1}"#.to_string(),
		// ),
		// (
		// 	r#"{"jsonrpc": "2.0", "method": "chain_getTransactionInTxPool", "params": ["0x3b624b93cb726681ddb8d79378783eb2b3147804"], "id": 1}"#.to_string(),
		// 	r#"{"jsonrpc":"2.0","result":{"hash":"0x3b624b93cb726681ddb8d79378783eb2b3147804","witness":null,"call":{"module":"system","method":"init","params":"0x0e00000000000000636861696e2d6a64726a71666868a0f79e5e"}},"id":1}"#.to_string(),
		// )
	]
}

fn get_support() -> DefaultApiSupport<Chain> {
	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	init(&home);

	let chain_config = ChainConfig { home };

	let chain = Arc::new(Chain::new(chain_config).unwrap());

	let txpool_config = TxPoolConfig {
		pool_capacity: 32,
		buffer_capacity: 32,
	};

	let txpool = Arc::new(TxPool::new(txpool_config, chain.clone()).unwrap());

	DefaultApiSupport::new(chain, txpool)
}

fn init(home: &PathBuf) {
	let config_path = home.join("config");

	fs::create_dir_all(&config_path).unwrap();

	let spec = r#"
[basic]
hash = "blake2b_160"
dsa = "ed25519"
address = "blake2b_160"

[genesis]

# System module init
[[genesis.txs]]
module = "system"
method = "init"
params = '''
{
    "chain_id": "chain-test",
    "timestamp": "2020-04-16T23:46:02.189+08:00"
}
'''
	"#;

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
