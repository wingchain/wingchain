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

use serde::Serialize;
use tokio::time::Duration;

use crypto::hash::{Hash as HashT, HashImpl};
use node_txpool::support::TxPoolSupport;
use node_txpool::{PoolTransaction, TxPool, TxPoolConfig};
use primitives::errors::{CommonError, CommonErrorKind, CommonResult, Display};
use primitives::{codec, Call, Hash, Params, Transaction};

#[derive(Clone)]
struct TestTxPoolSupport {
	hash: Arc<HashImpl>,
}

#[derive(Debug, Display)]
enum ErrorKind {
	#[display(fmt = "Invalid module: {}", _0)]
	InvalidModule(String),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::TxPool, Box::new(error))
	}
}

impl TxPoolSupport for TestTxPoolSupport {
	fn hash<E: Serialize>(&self, data: &E) -> CommonResult<Hash> {
		let mut out = vec![0u8; self.hash.length().into()];
		self.hash.hash(&mut out, &codec::encode(data)?);
		Ok(Hash(out))
	}
	fn validate_tx(&self, tx: &Transaction) -> CommonResult<()> {
		if tx.call.module.as_str() == "b" {
			return Err(ErrorKind::InvalidModule(tx.call.module.clone()).into());
		}
		Ok(())
	}
}

#[tokio::test]
async fn test_txpool() {
	let support = Arc::new(get_support());
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module: "a".to_string(),
			method: "a".to_string(),
			params: Params(vec![1u8; 32]),
		},
	};

	let expected_queue = vec![Arc::new(PoolTransaction {
		tx: Arc::new(tx.clone()),
		tx_hash: support.hash(&tx.clone()).unwrap(),
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
	let support = Arc::new(get_support());
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module: "a".to_string(),
			method: "a".to_string(),
			params: Params(vec![1u8; 32]),
		},
	};

	txpool.insert(tx.clone()).await.unwrap();

	let result = txpool.insert(tx).await;
	assert!(format!("{:?}", result).contains("error: Duplicated"));
}

#[tokio::test]
async fn test_txpool_validate() {
	let support = Arc::new(get_support());
	let config = TxPoolConfig {
		pool_capacity: 1024,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module: "b".to_string(),
			method: "a".to_string(),
			params: Params(vec![1u8; 32]),
		},
	};

	let result = txpool.insert(tx.clone()).await;
	assert!(format!("{:?}", result).contains("error: InvalidModule"));
}

#[tokio::test]
async fn test_txpool_capacity() {
	let support = Arc::new(get_support());
	let config = TxPoolConfig {
		pool_capacity: 2,
		buffer_capacity: 256,
	};
	let txpool = TxPool::new(config, support.clone()).unwrap();

	let tx = Transaction {
		witness: None,
		call: Call {
			module: "a".to_string(),
			method: "a".to_string(),
			params: Params(vec![1u8; 32]),
		},
	};

	let tx2 = {
		let mut tx = tx.clone();
		tx.call.module = "c".to_string();
		tx
	};

	let tx3 = {
		let mut tx = tx.clone();
		tx.call.module = "d".to_string();
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
