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

use async_trait::async_trait;
use node_chain::Chain;
use node_txpool::support::TxPoolSupport;
use node_txpool::TxPool;
use primitives::errors::CommonResult;
use primitives::{Address, Block, BlockNumber, Call, Hash, Header, Transaction, TransactionResult};

#[async_trait]
pub trait ApiSupport {
	async fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash>;
	async fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>>;
	async fn get_confirmed_executed_number(&self) -> CommonResult<Option<BlockNumber>>;
	async fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	async fn get_block(&self, block_hash: &Hash) -> CommonResult<Option<Block>>;
	async fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	async fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	async fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>>;
	async fn insert_transaction(&self, transaction: Transaction) -> CommonResult<()>;
	async fn get_transaction_in_txpool(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	async fn execute_call(
		&self,
		block_hash: &Hash,
		sender: Option<&Address>,
		call: &Call,
	) -> CommonResult<TransactionResult>;
}

pub struct DefaultApiSupport<TS>
where
	TS: TxPoolSupport,
{
	chain: Arc<Chain>,
	txpool: Arc<TxPool<TS>>,
}

impl<TS> DefaultApiSupport<TS>
where
	TS: TxPoolSupport,
{
	pub fn new(chain: Arc<Chain>, txpool: Arc<TxPool<TS>>) -> Self {
		Self { chain, txpool }
	}
}

#[async_trait]
impl<TS> ApiSupport for DefaultApiSupport<TS>
where
	TS: TxPoolSupport + Send + Sync,
{
	async fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.chain.hash_transaction(tx)
	}
	async fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_confirmed_number()
	}
	async fn get_confirmed_executed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_confirmed_executed_number()
	}
	async fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>> {
		self.chain.get_block_hash(number)
	}
	async fn get_block(&self, block_hash: &Hash) -> CommonResult<Option<Block>> {
		self.chain.get_block(block_hash)
	}
	async fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>> {
		self.chain.get_header(block_hash)
	}
	async fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.chain.get_transaction(tx_hash)
	}
	async fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>> {
		self.chain.get_raw_transaction(tx_hash)
	}
	async fn insert_transaction(&self, tx: Transaction) -> CommonResult<()> {
		self.txpool.insert(tx).await
	}
	async fn get_transaction_in_txpool(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		let tx = match self.txpool.get_map().get(tx_hash) {
			Some(tx) => tx.tx.clone(),
			None => return Ok(None),
		};
		Ok(Some(tx))
	}
	async fn execute_call(
		&self,
		block_hash: &Hash,
		sender: Option<&Address>,
		call: &Call,
	) -> CommonResult<TransactionResult> {
		self.chain.execute_call(block_hash, sender, call)
	}
}
