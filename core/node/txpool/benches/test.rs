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
use test::{black_box, Bencher};

use futures::future::join_all;
use tempfile::tempdir;
use tokio::runtime::Runtime;

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{Dsa, DsaImpl, KeyPair, KeyPairImpl};
use node_chain::{Chain, ChainConfig};
use node_executor::{module, Executor};
use node_txpool::{TxPool, TxPoolConfig};
use primitives::{codec, Address, PublicKey, Signature, Transaction, Witness};

const TXS_SIZE: usize = 2000;

#[bench]
fn bench_txpool_insert_transfer(b: &mut Bencher) {
	let txs = gen_transfer_txs(TXS_SIZE);
	let support = get_support();
	bench_txpool_insert_txs(b, support, txs);
}

fn bench_txpool_insert_txs(b: &mut Bencher, support: Arc<Chain>, txs: Vec<Transaction>) {
	b.iter(|| {
		black_box({
			let config = TxPoolConfig {
				pool_capacity: 10240,
				buffer_capacity: 10240,
			};

			let mut rt = Runtime::new().unwrap();
			rt.block_on(async {
				let txpool = TxPool::new(config, support.clone()).unwrap();
				let futures = txs
					.iter()
					.map(|tx| txpool.insert(tx.clone()))
					.collect::<Vec<_>>();
				join_all(futures).await;
			});
		})
	});
}

fn get_support() -> Arc<Chain> {
	let path = tempdir().expect("could not create a temp dir");
	let home = path.into_path();

	init(&home);

	let chain_config = ChainConfig { home };

	let chain = Arc::new(Chain::new(chain_config).unwrap());
	chain
}

fn gen_transfer_txs(size: usize) -> Vec<Transaction> {
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

	let mut txs = Vec::with_capacity(size);
	for nonce in 0..size {
		let tx = gen_transfer_tx(nonce as u32, sig_len, &executor, &account1, &account2);
		txs.push(tx);
	}
	txs
}

fn gen_transfer_tx(
	nonce: u32,
	sig_len: usize,
	executor: &Executor,
	account1: &(Vec<u8>, PublicKey, KeyPairImpl, Address),
	account2: &(Vec<u8>, PublicKey, KeyPairImpl, Address),
) -> Transaction {
	let until = 1u32;
	let mut tx = executor
		.build_tx(
			"balance".to_string(),
			"transfer".to_string(),
			module::balance::TransferParams {
				recipient: account2.3.clone(),
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
		public_key: account1.1.clone(),
		signature: sig,
		nonce,
		until,
	});
	executor.validate_tx(&tx).unwrap();
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
