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

use serde::Serialize;
use tempfile::tempdir;

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{Dsa, DsaImpl, KeyPair};
use crypto::hash::{Hash as HashT, HashImpl};
use node_api::support::DefaultApiSupport;
use node_api::{Api, ApiConfig};
use node_chain::{Chain, ChainConfig};
use node_executor::{module, Executor};
use node_txpool::{TxPool, TxPoolConfig};
use primitives::{
	codec, Address, Hash, PublicKey, Signature, Transaction, TransactionForHash, Witness,
};

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
	let tx = get_tx();

	let tx_hash = hash(TransactionForHash::new(&tx));

	let tx_hex = hex::encode(codec::encode(&tx).unwrap());

	let tx_hash_hex = hex::encode(tx_hash.0);
	let tx_public_key_hex = hex::encode(&tx.witness.clone().unwrap().public_key.0);
	let tx_sig_hex = hex::encode(&tx.witness.clone().unwrap().signature.0);
	let nonce_hex = hex::encode(tx.witness.clone().unwrap().nonce.to_be_bytes());
	let until_hex = hex::encode(tx.witness.clone().unwrap().until.to_be_bytes());
	let params_hex = hex::encode(&tx.call.params.0);

	vec![
		(
			r#"{"jsonrpc": "2.0", "method": "chain_getBlockByNumber", "params": ["best"], "id": 1}"#
				.to_string(),
			r#"{"jsonrpc":"2.0","result":{"hash":"0xdf57d2ec74a0eab9cb11e223ec8f4cb44b1c407d16eed3eee00434d8521b7996","header":{"number":"0x00000000","timestamp":"0x5ea93208","parent_hash":"0x0000000000000000000000000000000000000000000000000000000000000000","meta_txs_root":"0xd9ff49df9fb0d4abd0926af1c32059d63b62f61cfe117041d71f56e96d8d5ad4","meta_state_root":"0xf29d7ef97517727c206ffb819977017fd545ee02dc82e4d0d703faf590a401f4","payload_txs_root":"0xe67d3833c74144ef5d7df2c0a0aacf5b97f59796556efe292228bb6e089d8ead","payload_executed_gap":"0x01","payload_executed_state_root":"0x0000000000000000000000000000000000000000000000000000000000000000"},"body":{"meta_txs":["0x1fd2851248c12938fa5d59da8513e3471755a1c797f21d1f41bc013050b07954"],"payload_txs":["0x3d8d2b91d2cd54c27d59d954036fb64963894c84415f89cc0b3c5c49f25c370b"]}},"id":1}"#.to_string(),
		),
		(
			r#"{"jsonrpc": "2.0", "method": "chain_getTransactionByHash", "params": ["0x1fd2851248c12938fa5d59da8513e3471755a1c797f21d1f41bc013050b07954"], "id": 1}"#
				.to_string(),
			r#"{"jsonrpc":"2.0","result":{"hash":"0x1fd2851248c12938fa5d59da8513e3471755a1c797f21d1f41bc013050b07954","witness":null,"call":{"module":"system","method":"init","params":"0x0a00000000000000636861696e2d746573740832a95e"}},"id":1}"#.to_string(),
		),
		(
			r#"{"jsonrpc": "2.0", "method": "chain_getRawTransactionByHash", "params": ["0x1fd2851248c12938fa5d59da8513e3471755a1c797f21d1f41bc013050b07954"], "id": 1}"#.to_string(),
			r#"{"jsonrpc":"2.0","result":"0x00060000000000000073797374656d0400000000000000696e697416000000000000000a00000000000000636861696e2d746573740832a95e","id":1}"#.to_string(),
		),
		(
			format!(r#"{{"jsonrpc": "2.0", "method": "chain_sendRawTransaction", "params": ["0x{}"], "id": 1}}"#, tx_hex),
			format!(r#"{{"jsonrpc":"2.0","result":"0x{}","id":1}}"#, tx_hash_hex),
		),
		(
			format!(r#"{{"jsonrpc": "2.0", "method": "chain_getTransactionInTxPool", "params": ["{}"], "id": 1}}"#, tx_hash_hex),
			format!(r#"{{"jsonrpc":"2.0","result":{{"hash":"0x{}","witness":{{"public_key":"0x{}","signature":"0x{}","nonce":"0x{}","until":"0x{}"}},"call":{{"module":"balance","method":"transfer","params":"0x{}"}}}},"id":1}}"#,
					tx_hash_hex, tx_public_key_hex, tx_sig_hex, nonce_hex, until_hex, params_hex),
		)
	]
}

fn hash<E: Serialize>(data: E) -> Hash {
	let hasher = HashImpl::Blake2b256;
	let mut hash = vec![0u8; hasher.length().into()];
	hasher.hash(&mut hash, &codec::encode(&data).unwrap());
	Hash(hash)
}

fn get_tx() -> Transaction {
	let dsa = Arc::new(DsaImpl::Ed25519);
	let address = Arc::new(AddressImpl::Blake2b160);
	let executor = Executor::new(dsa.clone(), address.clone());

	let (secret_key_len, public_key_len, sig_len) = dsa.length().into();
	let address_len = address.length().into();

	let account1 = {
		let secret_key = vec![1u8; secret_key_len];

		let key_pair = dsa.key_pair_from_secret_key(&secret_key).unwrap();
		let public_key = PublicKey({
			let mut out = vec![0u8; public_key_len];
			key_pair.public_key(&mut out);
			out
		});
		let account = Address({
			let mut out = vec![0u8; address_len];
			address.address(&mut out, &public_key.0);
			out
		});
		(secret_key, public_key, key_pair, account)
	};

	let account2 = {
		let secret_key = vec![2u8; secret_key_len];

		let key_pair = dsa.key_pair_from_secret_key(&secret_key).unwrap();
		let public_key = PublicKey({
			let mut out = vec![0u8; public_key_len];
			key_pair.public_key(&mut out);
			out
		});
		let account = Address({
			let mut out = vec![0u8; address_len];
			address.address(&mut out, &public_key.0);
			out
		});
		(secret_key, public_key, key_pair, account)
	};

	let nonce = 0u32;
	let until = 1u32;
	let mut tx = executor
		.build_tx(
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3,
				value: 2,
			},
		)
		.unwrap();
	let message = codec::encode(&(nonce, until, &tx.call)).unwrap();
	let sig = Signature({
		let mut out = vec![0u8; sig_len];
		account1.2.sign(&message, &mut out);
		out
	});
	tx.witness = Some(Witness {
		public_key: account1.1,
		signature: sig,
		nonce,
		until,
	});
	executor.validate_tx(&tx).unwrap();
	tx
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
hash = "blake2b_256"
dsa = "ed25519"
address = "blake2b_160"

[genesis]

[[genesis.txs]]
module = "system"
method = "init"
params = '''
{
    "chain_id": "chain-test",
    "timestamp": "2020-04-29T15:51:36.502+08:00"
}
'''

[[genesis.txs]]
module = "balance"
method = "init"
params = '''
{
    "endow": [
    	["b4decd5a5f8f2ba708f8ced72eec89f44f3be96a", 10]
    ]
}
'''
	"#;

	fs::write(config_path.join("spec.toml"), &spec).unwrap();
}
