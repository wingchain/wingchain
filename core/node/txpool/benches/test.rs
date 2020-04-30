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

#![feature(test)]

extern crate test;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use test::{Bencher, black_box};

use futures::future::join_all;
use tempfile::tempdir;
use tokio::runtime::Runtime;

use crypto::dsa::KeyPairImpl;
use node_chain::{Chain, ChainConfig, module};
use node_txpool::{TxPool, TxPoolConfig};
use primitives::{Address, PublicKey, SecretKey, Transaction};
use utils_test::test_accounts;

const TXS_SIZE: usize = 2000;

#[bench]
fn bench_txpool_insert_transfer(b: &mut Bencher) {
	let chain = get_chain();
	let txs = gen_transfer_txs(&chain, TXS_SIZE);
	bench_txpool_insert_txs(b, chain, txs);
}

fn bench_txpool_insert_txs(b: &mut Bencher, chain: Arc<Chain>, txs: Vec<Transaction>) {
	b.iter(|| {
		black_box({
			let config = TxPoolConfig {
				pool_capacity: 10240,
				buffer_capacity: 10240,
			};

			let mut rt = Runtime::new().unwrap();
			rt.block_on(async {
				let txpool = TxPool::new(config, chain.clone()).unwrap();
				let futures = txs
					.iter()
					.map(|tx| txpool.insert(tx.clone()))
					.collect::<Vec<_>>();
				let r = join_all(futures).await;
				println!("{:?}", r);
			});
		})
	});
}

fn get_chain() -> Arc<Chain> {
	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	init(&home);

	let chain_config = ChainConfig { home };

	let chain = Arc::new(Chain::new(chain_config).unwrap());
	chain
}

fn gen_transfer_txs(chain: &Arc<Chain>, size: usize) -> Vec<Transaction> {

	let (account1, account2) = test_accounts(
		chain.get_basic().dsa.clone(),
		chain.get_basic().address.clone(),
	);

	let mut txs = Vec::with_capacity(size);
	for nonce in 0..size {
		let tx = gen_transfer_tx(&chain, nonce as u32, &account1, &account2);
		txs.push(tx);
	}
	txs
}

fn gen_transfer_tx(
	chain: &Arc<Chain>,
	nonce: u32,
	account1: &(SecretKey, PublicKey, KeyPairImpl, Address),
	account2: &(SecretKey, PublicKey, KeyPairImpl, Address),
) -> Transaction {
	let until = 1u32;
	let tx = chain
		.build_transaction(
			Some((account1.0.clone(), nonce, until)),
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
				value: 2,
			},
		)
		.unwrap();
	chain.validate_transaction(&tx, true).unwrap();
	tx
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
