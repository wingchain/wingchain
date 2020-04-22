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

use chashmap::CHashMap;
use log::info;
use parking_lot::RwLock;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use primitives::errors::CommonResult;
use primitives::{Hash, Transaction};

use crate::support::TxPoolSupport;

pub mod errors;
pub mod support;

pub struct TxPoolConfig {
	pub pool_capacity: usize,
	pub buffer_capacity: usize,
}

#[derive(Debug, PartialEq)]
pub struct PoolTransaction {
	pub tx: Arc<Transaction>,
	pub tx_hash: Hash,
}

pub struct TxPool<S>
where
	S: TxPoolSupport,
{
	config: TxPoolConfig,
	support: Arc<S>,
	map: CHashMap<Hash, Arc<PoolTransaction>>,
	queue: Arc<RwLock<Vec<Arc<PoolTransaction>>>>,
	buffer_tx: Sender<Arc<PoolTransaction>>,
}

impl<S> TxPool<S>
where
	S: TxPoolSupport,
{
	pub fn new(config: TxPoolConfig, support: Arc<S>) -> CommonResult<Self> {
		let map = CHashMap::with_capacity(config.pool_capacity);
		let queue = Arc::new(RwLock::new(Vec::with_capacity(config.pool_capacity)));

		let (buffer_tx, buffer_rx) = channel(config.buffer_capacity);

		let tx_pool = Self {
			config,
			support,
			map,
			queue: queue.clone(),
			buffer_tx,
		};

		tokio::spawn(process_buffer(buffer_rx, queue));

		info!("Initializing txpool");

		Ok(tx_pool)
	}

	pub fn get_queue(&self) -> &Arc<RwLock<Vec<Arc<PoolTransaction>>>> {
		&self.queue
	}

	pub fn get_map(&self) -> &CHashMap<Hash, Arc<PoolTransaction>> {
		&self.map
	}

	pub async fn insert(&self, tx: Transaction) -> CommonResult<()> {
		self.check_capacity()?;
		let tx_hash = self.support.hash(&tx)?;
		self.check_exist(&tx_hash)?;

		self.support.validate_tx(&tx)?;

		let pool_tx = Arc::new(PoolTransaction {
			tx: Arc::new(tx),
			tx_hash: tx_hash.clone(),
		});

		if self.map.insert(tx_hash.clone(), pool_tx.clone()).is_some() {
			return Err(errors::ErrorKind::Duplicated(tx_hash.clone()).into());
		}

		let result = self.buffer_tx.clone().send(pool_tx).await;

		if let Err(_e) = result {
			self.map.remove(&tx_hash);
			return Err(errors::ErrorKind::Insert(tx_hash).into());
		}

		Ok(())
	}

	fn check_capacity(&self) -> CommonResult<()> {
		if self.map.len() >= self.config.pool_capacity {
			return Err(errors::ErrorKind::ExceedCapacity(self.config.pool_capacity).into());
		}
		Ok(())
	}

	fn check_exist(&self, tx_hash: &Hash) -> CommonResult<()> {
		if self.contain(tx_hash) {
			return Err(errors::ErrorKind::Duplicated(tx_hash.clone()).into());
		}
		Ok(())
	}

	fn contain(&self, tx_hash: &Hash) -> bool {
		self.map.contains_key(tx_hash)
	}
}

async fn process_buffer(
	buffer_rx: Receiver<Arc<PoolTransaction>>,
	queue: Arc<RwLock<Vec<Arc<PoolTransaction>>>>,
) {
	let mut buffer_rx = buffer_rx;
	loop {
		let tx = buffer_rx.recv().await;
		if let Some(tx) = tx {
			queue.write().push(tx);
		}
	}
}
