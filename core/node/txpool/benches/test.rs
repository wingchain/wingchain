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

use std::sync::Arc;
use test::{black_box, Bencher};

use codec::Encode;
use futures::future::join_all;
use rand::random;
use tokio::runtime::Runtime;
use tokio::time::Duration;

use crypto::hash::{Hash as HashT, HashImpl};
use node_txpool::support::TxPoolSupport;
use node_txpool::{Config, TxPool};
use primitives::errors::CommonResult;
use primitives::{Call, DispatchId, Hash, Params, Transaction};

const TXS_SIZE: usize = 10000;

#[bench]
fn bench_txpool_insert_32(b: &mut Bencher) {
	bench_txpool_insert_with_params_size(b, 32);
}

#[bench]
fn bench_txpool_insert_64(b: &mut Bencher) {
	bench_txpool_insert_with_params_size(b, 64);
}

#[bench]
fn bench_txpool_insert_512(b: &mut Bencher) {
	bench_txpool_insert_with_params_size(b, 512);
}

fn bench_txpool_insert_with_params_size(b: &mut Bencher, params_size: usize) {
	let txs = gen_txs(TXS_SIZE, params_size);

	let run = move || {};

	b.iter(|| {
		black_box({
			let support = get_support();
			let config = Config {
				pool_capacity: 10240,
				buffer_capacity: 10240,
			};

			let mut rt = Runtime::new().unwrap();
			rt.block_on(async {
				let txpool = TxPool::new(config, support).unwrap();
				let futures = txs
					.iter()
					.map(|tx| txpool.insert(tx.clone()))
					.collect::<Vec<_>>();
				join_all(futures).await;
			});
		})
	});
}

#[derive(Clone)]
struct TestTxPoolSupport {
	hash: Arc<HashImpl>,
}

impl TxPoolSupport for TestTxPoolSupport {
	fn hash<E: Encode>(&self, data: &E) -> Hash {
		let mut out = vec![0u8; self.hash.length().into()];
		self.hash.hash(&mut out, &data.encode());
		Hash(out)
	}
	fn validate_tx(&self, tx: &Transaction) -> CommonResult<()> {
		Ok(())
	}
}

fn get_support() -> TestTxPoolSupport {
	let hash = Arc::new(HashImpl::Blake2b256);
	let support = TestTxPoolSupport { hash };
	support
}

fn gen_txs(size: usize, params_size: usize) -> Vec<Transaction> {
	let mut txs = Vec::with_capacity(size);
	for _ in 0..size {
		let params = Params((0..params_size).map(|_| random::<u8>()).collect());
		let tx = Transaction {
			witness: None,
			call: Call {
				module_id: DispatchId([1u8, 0, 0, 0]),
				method_id: DispatchId([1u8, 0, 0, 0]),
				params,
			},
		};
		txs.push(tx);
	}
	txs
}