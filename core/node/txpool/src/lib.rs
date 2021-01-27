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

//! Transaction pool

use std::collections::HashSet;
use std::sync::Arc;

use chashmap::CHashMap;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use log::{info, trace};
use parking_lot::RwLock;

use primitives::errors::{Catchable, CommonResult};
use primitives::{FullTransaction, Hash, Transaction};

use crate::errors::InsertError;
use crate::support::TxPoolSupport;

pub mod errors;
pub mod support;

pub struct TxPoolConfig {
	/// Max transaction count
	pub pool_capacity: usize,
}

pub enum TxPoolOutMessage {
	TxInserted { tx_hash: Hash },
}

pub struct TxPool<S>
where
	S: TxPoolSupport,
{
	config: TxPoolConfig,
	support: Arc<S>,
	map: CHashMap<Hash, Arc<FullTransaction>>,
	queue: Arc<RwLock<Vec<Arc<FullTransaction>>>>,
	buffer_tx: UnboundedSender<Arc<FullTransaction>>,
	message_tx: UnboundedSender<TxPoolOutMessage>,
	message_rx: RwLock<Option<UnboundedReceiver<TxPoolOutMessage>>>,
}

impl<S> TxPool<S>
where
	S: TxPoolSupport,
{
	/// Create a new transaction pool
	pub fn new(config: TxPoolConfig, support: Arc<S>) -> CommonResult<Self> {
		let map = CHashMap::with_capacity(config.pool_capacity);
		let queue = Arc::new(RwLock::new(Vec::with_capacity(config.pool_capacity)));

		let (buffer_tx, buffer_rx) = unbounded();

		let (message_tx, message_rx) = unbounded();

		let txpool = Self {
			config,
			support,
			map,
			queue: queue.clone(),
			buffer_tx,
			message_tx,
			message_rx: RwLock::new(Some(message_rx)),
		};

		BufferStream::spawn(buffer_rx, queue);

		info!("Initializing txpool");

		Ok(txpool)
	}

	/// Get the queue of the pool
	/// The queue keep the transaction in the insertion order
	pub fn get_queue(&self) -> &Arc<RwLock<Vec<Arc<FullTransaction>>>> {
		&self.queue
	}

	/// Get the map of the pool
	/// The map is used to check if the pool already contains a transaction
	pub fn get_map(&self) -> &CHashMap<Hash, Arc<FullTransaction>> {
		&self.map
	}

	/// Insert a transaction into the pool
	pub fn insert(&self, tx: Transaction) -> CommonResult<()> {
		self.check_capacity()?;
		let tx_hash = self.support.hash_transaction(&tx)?;

		self.check_pool_exist(&tx_hash)?;
		self.validate_transaction(&tx_hash, &tx)?;

		let pool_tx = Arc::new(FullTransaction {
			tx,
			tx_hash: tx_hash.clone(),
		});

		if self.map.insert(tx_hash.clone(), pool_tx.clone()).is_some() {
			return Err(
				errors::ErrorKind::InsertError(InsertError::DuplicatedTx(format!("{}", tx_hash)))
					.into(),
			);
		}

		let result = self.buffer_tx.clone().unbounded_send(pool_tx);

		if let Err(e) = result {
			self.map.remove(&tx_hash);
			return Err(errors::ErrorKind::Channel(Box::new(e)).into());
		}

		self.on_tx_inserted(tx_hash)?;

		Ok(())
	}

	/// Remove transactions of given set of transaction hash
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

	/// Out message receiver
	pub fn message_rx(&self) -> Option<UnboundedReceiver<TxPoolOutMessage>> {
		self.message_rx.write().take()
	}

	/// Check pool capacify
	fn check_capacity(&self) -> CommonResult<()> {
		if self.map.len() >= self.config.pool_capacity {
			return Err(errors::ErrorKind::InsertError(InsertError::ExceedCapacity(
				self.config.pool_capacity,
			))
			.into());
		}
		Ok(())
	}

	/// Check if the pool already contains a transaction
	fn check_pool_exist(&self, tx_hash: &Hash) -> CommonResult<()> {
		if self.contain(tx_hash) {
			return Err(
				errors::ErrorKind::InsertError(InsertError::DuplicatedTx(format!("{}", tx_hash)))
					.into(),
			);
		}
		Ok(())
	}

	fn on_tx_inserted(&self, tx_hash: Hash) -> CommonResult<()> {
		trace!("Tx inserted: {}", tx_hash);
		self.message_tx
			.unbounded_send(TxPoolOutMessage::TxInserted { tx_hash })
			.map_err(|e| errors::ErrorKind::Channel(Box::new(e)))?;
		Ok(())
	}

	/// Validate a transaction
	fn validate_transaction(&self, tx_hash: &Hash, tx: &Transaction) -> CommonResult<()> {
		self.support
			.validate_transaction(tx_hash, &tx, true)
			.or_else_catch::<node_chain::errors::ErrorKind, _>(|e| match e {
				node_chain::errors::ErrorKind::ValidateTxError(e) => Some(Err(
					errors::ErrorKind::InsertError(errors::InsertError::InvalidTx(e.clone()))
						.into(),
				)),
				_ => None,
			})?;
		Ok(())
	}

	/// Check if the pool contains the transaction
	fn contain(&self, tx_hash: &Hash) -> bool {
		self.map.contains_key(tx_hash)
	}
}

struct BufferStream {
	buffer_rx: UnboundedReceiver<Arc<FullTransaction>>,
	queue: Arc<RwLock<Vec<Arc<FullTransaction>>>>,
}

impl BufferStream {
	fn spawn(
		buffer_rx: UnboundedReceiver<Arc<FullTransaction>>,
		queue: Arc<RwLock<Vec<Arc<FullTransaction>>>>,
	) {
		let this = Self { buffer_rx, queue };
		tokio::spawn(this.start());
	}
	async fn start(mut self) {
		loop {
			let tx = self.buffer_rx.next().await;
			match tx {
				Some(tx) => self.queue.write().push(tx),
				None => break,
			}
		}
	}
}
