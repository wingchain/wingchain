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
use node_txpool::{PoolTransaction, TxPool};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::{BlockNumber, Hash, Header};

#[async_trait]
pub trait ConsensusSupport {
	fn get_best_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<R>;
	// fn build_block(&self, timestamp: )
	fn get_transactions_in_txpool(&self) -> CommonResult<Vec<Arc<PoolTransaction>>>;
}

pub struct DefaultConsensusSupport<TS>
where
	TS: TxPoolSupport,
{
	chain: Arc<Chain>,
	#[allow(dead_code)]
	txpool: Arc<TxPool<TS>>,
}

impl<TS> DefaultConsensusSupport<TS>
where
	TS: TxPoolSupport,
{
	pub fn new(chain: Arc<Chain>, txpool: Arc<TxPool<TS>>) -> Self {
		Self { chain, txpool }
	}
}

#[async_trait]
impl<TS> ConsensusSupport for DefaultConsensusSupport<TS>
where
	TS: TxPoolSupport + Send + Sync,
{
	fn get_best_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_best_number()
	}
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>> {
		self.chain.get_block_hash(number)
	}
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>> {
		self.chain.get_header(block_hash)
	}
	fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<R> {
		self.chain
			.execute_call_with_block_number(block_number, module, method, params)
	}
	fn get_transactions_in_txpool(&self) -> CommonResult<Vec<Arc<PoolTransaction>>> {
		let txs = (*self.txpool.get_queue().read()).clone();
		Ok(txs)
	}
}
