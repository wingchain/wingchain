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

use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::errors::CommonResult;
use primitives::{FullTransaction, Hash, Transaction};

use crate::support::TxPoolSupport;
use std::collections::HashSet;

pub mod errors;
pub mod support;

pub struct TxPoolConfig {
	pub pool_capacity: usize,
	pub buffer_capacity: usize,
}

pub struct TxPool<S>
where
	S: TxPoolSupport,
{
	config: TxPoolConfig,
	system_meta: module::system::Meta,
	support: Arc<S>,
	map: CHashMap<Hash, Arc<FullTransaction>>,
	queue: Arc<RwLock<Vec<Arc<FullTransaction>>>>,
	buffer_tx: Sender<Arc<FullTransaction>>,
}

impl<S> TxPool<S>
where
	S: TxPoolSupport,
{
	pub fn new(config: TxPoolConfig, support: Arc<S>) -> CommonResult<Self> {
		let map = CHashMap::with_capacity(config.pool_capacity);
		let queue = Arc::new(RwLock::new(Vec::with_capacity(config.pool_capacity)));

		let (buffer_tx, buffer_rx) = channel(config.buffer_capacity);

		let system_meta = get_system_meta(support.clone())?;

		let tx_pool = Self {
			config,
			system_meta,
			support,
			map,
			queue: queue.clone(),
			buffer_tx,
		};

		tokio::spawn(process_buffer(buffer_rx, queue));

		info!("Initializing txpool");

		Ok(tx_pool)
	}

	pub fn get_queue(&self) -> &Arc<RwLock<Vec<Arc<FullTransaction>>>> {
		&self.queue
	}

	pub fn get_map(&self) -> &CHashMap<Hash, Arc<FullTransaction>> {
		&self.map
	}

	pub async fn insert(&self, tx: Transaction) -> CommonResult<()> {
		self.check_capacity()?;
		let tx_hash = self.support.hash_transaction(&tx)?;

		self.check_pool_exist(&tx_hash)?;
		self.validate_transaction(&tx_hash, &tx)?;
		self.check_chain_exist(&tx_hash)?;

		let pool_tx = Arc::new(FullTransaction {
			tx,
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

	pub fn remove(&self, tx_hash_set: &HashSet<Hash>) -> CommonResult<()> {
		{
			let mut queue = self.queue.write();
			queue.retain(|x| !tx_hash_set.contains(&x.tx_hash));
		}

		{
			self.map.retain(|k, _v| !tx_hash_set.contains(k));
		}

		Ok(())
	}

	fn check_capacity(&self) -> CommonResult<()> {
		if self.map.len() >= self.config.pool_capacity {
			return Err(errors::ErrorKind::ExceedCapacity(self.config.pool_capacity).into());
		}
		Ok(())
	}

	fn check_pool_exist(&self, tx_hash: &Hash) -> CommonResult<()> {
		if self.contain(tx_hash) {
			return Err(errors::ErrorKind::Duplicated(tx_hash.clone()).into());
		}
		Ok(())
	}

	fn check_chain_exist(&self, tx_hash: &Hash) -> CommonResult<()> {
		if self.support.get_transaction(&tx_hash)?.is_some() {
			return Err(errors::ErrorKind::Duplicated(tx_hash.clone()).into());
		}
		Ok(())
	}

	fn validate_transaction(&self, tx_hash: &Hash, tx: &Transaction) -> CommonResult<()> {
		self.support.validate_transaction(&tx, true)?;
		let witness = tx.witness.as_ref().expect("qed");

		let confirmed_number = self.support.get_confirmed_number()?.expect("qed");
		let until_gap = self.system_meta.until_gap;
		let max_until = confirmed_number + until_gap;

		if witness.until > max_until {
			return Err(errors::ErrorKind::InvalidUntil(tx_hash.clone()).into());
		}

		if witness.until <= confirmed_number {
			return Err(errors::ErrorKind::ExceedUntil(tx_hash.clone()).into());
		}

		Ok(())
	}

	fn contain(&self, tx_hash: &Hash) -> bool {
		self.map.contains_key(tx_hash)
	}
}

async fn process_buffer(
	buffer_rx: Receiver<Arc<FullTransaction>>,
	queue: Arc<RwLock<Vec<Arc<FullTransaction>>>>,
) {
	let mut buffer_rx = buffer_rx;
	loop {
		let tx = buffer_rx.recv().await;
		if let Some(tx) = tx {
			queue.write().push(tx);
		}
	}
}

fn get_system_meta<S: TxPoolSupport>(support: Arc<S>) -> CommonResult<module::system::Meta> {
	let block_number = support.get_confirmed_number()?.expect("qed");
	support
		.execute_call_with_block_number(
			&block_number,
			None,
			"system".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}
