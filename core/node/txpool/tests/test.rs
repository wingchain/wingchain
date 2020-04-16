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

use std::error::Error;
use std::sync::Arc;

use parity_codec::Encode;
use tokio::time::Duration;

use crypto::hash::{Hash as HashT, HashImpl};
use node_txpool::support::TxPoolSupport;
use node_txpool::{Config, PoolTransaction, TxPool};
use primitives::errors::{CommonError, CommonErrorKind, CommonResult, Display};
use primitives::{Call, DispatchId, Hash, Params, Transaction};
use tokio::runtime::Runtime;

#[derive(Clone)]
struct TestTxPoolSupport {
	hash: Arc<HashImpl>,
}

#[derive(Debug, Display)]
enum ErrorKind {
	#[display(fmt = "Invalid dispatch id: {:?}", _0)]
	InvalidDispatchId(DispatchId),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::TxPool, Box::new(error))
	}
}

impl TxPoolSupport for TestTxPoolSupport {
	fn hash<E: Encode>(&self, data: &E) -> Hash {
		let mut out = vec![0u8; self.hash.length().into()];
		self.hash.hash(&mut out, &data.encode());
		Hash(out)
	}
	fn validate_tx(&self, tx: &Transaction) -> CommonResult<()> {
		if tx.call.module_id == DispatchId([2, 0, 0, 0]) {
			return Err(ErrorKind::InvalidDispatchId(tx.call.module_id.clone()).into());
		}
		Ok(())
	}
}

#[tokio::test]
async fn test_txpool() {
	let support = get_support();
	let config = Config {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module_id: DispatchId([1u8, 0, 0, 0]),
			method_id: DispatchId([1u8, 0, 0, 0]),
			params: Params(vec![1u8; 32]),
		},
	};

	let expected_queue = vec![Arc::new(PoolTransaction {
		tx: Arc::new(tx.clone()),
		tx_hash: support.hash(&tx.clone()),
	})];

	txpool.insert(tx).await.unwrap();

	loop {
		{
			let queue = txpool.get_queue().read();
			if queue.len() > 0 {
				assert_eq!(*queue, expected_queue);
				break;
			}
		}
		tokio::time::delay_for(Duration::from_millis(10)).await;
	}
}

#[tokio::test]
async fn test_txpool_dup() {
	let support = get_support();
	let config = Config {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module_id: DispatchId([1u8, 0, 0, 0]),
			method_id: DispatchId([1u8, 0, 0, 0]),
			params: Params(vec![1u8; 32]),
		},
	};

	txpool.insert(tx.clone()).await.unwrap();

	let result = txpool.insert(tx).await;
	assert!(format!("{:?}", result).contains("error: Duplicated"));
}

#[tokio::test]
async fn test_txpool_validate() {
	let support = get_support();
	let config = Config {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module_id: DispatchId([2u8, 0, 0, 0]),
			method_id: DispatchId([1u8, 0, 0, 0]),
			params: Params(vec![1u8; 32]),
		},
	};

	let result = txpool.insert(tx.clone()).await;
	assert!(format!("{:?}", result).contains("error: InvalidDispatchId"));
}

#[tokio::test]
async fn test_txpool_capacity() {
	let support = get_support();
	let config = Config {
		pool_capacity: 2,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module_id: DispatchId([1u8, 0, 0, 0]),
			method_id: DispatchId([1u8, 0, 0, 0]),
			params: Params(vec![1u8; 32]),
		},
	};

	let tx2 = {
		let mut tx = tx.clone();
		tx.call.module_id = DispatchId([1u8, 1, 0, 0]);
		tx
	};

	let tx3 = {
		let mut tx = tx.clone();
		tx.call.module_id = DispatchId([1u8, 2, 0, 0]);
		tx
	};

	txpool.insert(tx).await.unwrap();
	txpool.insert(tx2).await.unwrap();
	let result = txpool.insert(tx3).await;
	assert!(format!("{:?}", result).contains("error: ExceedCapacity"));
}

fn get_support() -> TestTxPoolSupport {
	let hash = Arc::new(HashImpl::Blake2b256);
	let support = TestTxPoolSupport { hash };
	support
}
