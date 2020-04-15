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
use parking_lot::RwLock;
use tokio::stream::StreamExt;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use primitives::errors::CommonResult;
use primitives::{Hash, Transaction};

use crate::support::TxPoolSupport;

pub mod errors;
mod support;

pub struct Config {
	pub pool_capacity: usize,
	pub buffer_capacity: usize,
}

pub struct PoolTransaction {
	tx: Arc<Transaction>,
	tx_hash: Hash,
}

pub struct TxPool<S>
where
	S: TxPoolSupport,
{
	config: Config,
	support: S,
	map: CHashMap<Hash, Arc<PoolTransaction>>,
	pool: Arc<RwLock<Vec<Arc<PoolTransaction>>>>,
	buffer_tx: Sender<Arc<PoolTransaction>>,
}

impl<S> TxPool<S>
where
	S: TxPoolSupport,
{
	pub fn new(config: Config, support: S) -> CommonResult<Self> {
		let map = CHashMap::with_capacity(config.pool_capacity);
		let pool = Arc::new(RwLock::new(Vec::with_capacity(config.pool_capacity)));

		let (buffer_tx, buffer_rx) = channel(config.buffer_capacity);

		let tx_pool = Self {
			config,
			support,
			map,
			pool: pool.clone(),
			buffer_tx,
		};

		tokio::spawn(process_buffer(buffer_rx, pool));

		Ok(tx_pool)
	}

	pub async fn insert(&self, tx: Transaction) -> CommonResult<()> {
		self.check_capacity()?;
		let tx_hash = self.support.get_tx_hash(&tx);
		self.check_exist(&tx_hash)?;

		self.support.validate_tx(&tx)?;

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
			return Err(errors::ErrorKind::Duplicated.into());
		}
		Ok(())
	}

	fn contain(&self, tx_hash: &Hash) -> bool {
		self.map.contains_key(tx_hash)
	}
}

async fn process_buffer(
	buffer_rx: Receiver<Arc<PoolTransaction>>,
	pool: Arc<RwLock<Vec<Arc<PoolTransaction>>>>,
) {
	let mut buffer_rx = buffer_rx.fuse();
	loop {
		let tx = buffer_rx.next().await;
		if let Some(tx) = tx {
			pool.write().push(tx);
		}
	}
}
