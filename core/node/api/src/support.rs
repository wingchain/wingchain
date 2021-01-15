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
use futures::channel::mpsc::UnboundedSender;

use node_chain::Chain;
use node_coordinator::support::DefaultCoordinatorSupport;
use node_coordinator::{Coordinator, CoordinatorInMessage};
use node_txpool::support::DefaultTxPoolSupport;
use node_txpool::TxPool;
use primitives::errors::CommonResult;
use primitives::{
	Address, Block, BlockNumber, Call, Hash, Header, Nonce, OpaqueCallResult, Proof, Receipt,
	SecretKey, Transaction,
};

#[async_trait]
pub trait ApiSupport {
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash>;
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_confirmed_executed_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	fn get_block(&self, block_hash: &Hash) -> CommonResult<Option<Block>>;
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	fn get_proof(&self, block_hash: &Hash) -> CommonResult<Option<Proof>>;
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>>;
	fn get_receipt(&self, tx_hash: &Hash) -> CommonResult<Option<Receipt>>;
	fn insert_transaction(&self, transaction: Transaction) -> CommonResult<()>;
	fn execute_call(
		&self,
		block_hash: &Hash,
		sender: Option<&Address>,
		call: &Call,
	) -> CommonResult<OpaqueCallResult>;
	fn build_transaction(
		&self,
		witness: Option<(SecretKey, Nonce, BlockNumber)>,
		call: Call,
	) -> CommonResult<Transaction>;
	fn txpool_get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	fn coordinator_tx(&self) -> CommonResult<UnboundedSender<CoordinatorInMessage>>;
}

pub struct DefaultApiSupport {
	chain: Arc<Chain>,
	txpool: Arc<TxPool<DefaultTxPoolSupport>>,
	coordinator: Arc<Coordinator<DefaultCoordinatorSupport>>,
}

impl DefaultApiSupport {
	pub fn new(
		chain: Arc<Chain>,
		txpool: Arc<TxPool<DefaultTxPoolSupport>>,
		coordinator: Arc<Coordinator<DefaultCoordinatorSupport>>,
	) -> Self {
		Self {
			chain,
			txpool,
			coordinator,
		}
	}
}

#[async_trait]
impl ApiSupport for DefaultApiSupport {
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.chain.hash_transaction(tx)
	}
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_confirmed_number()
	}
	fn get_confirmed_executed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_confirmed_executed_number()
	}
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>> {
		self.chain.get_block_hash(number)
	}
	fn get_block(&self, block_hash: &Hash) -> CommonResult<Option<Block>> {
		self.chain.get_block(block_hash)
	}
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>> {
		self.chain.get_header(block_hash)
	}
	fn get_proof(&self, block_hash: &Hash) -> CommonResult<Option<Proof>> {
		self.chain.get_proof(block_hash)
	}
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.chain.get_transaction(tx_hash)
	}
	fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>> {
		self.chain.get_raw_transaction(tx_hash)
	}
	fn get_receipt(&self, tx_hash: &Hash) -> CommonResult<Option<Receipt>> {
		self.chain.get_receipt(tx_hash)
	}
	fn insert_transaction(&self, tx: Transaction) -> CommonResult<()> {
		self.txpool.insert(tx)
	}
	fn execute_call(
		&self,
		block_hash: &Hash,
		sender: Option<&Address>,
		call: &Call,
	) -> CommonResult<OpaqueCallResult> {
		self.chain.execute_call(block_hash, sender, call)
	}

	fn build_transaction(
		&self,
		witness: Option<(SecretKey, Nonce, BlockNumber)>,
		call: Call,
	) -> CommonResult<Transaction> {
		self.chain.build_transaction(witness, call)
	}

	fn txpool_get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		let tx = match self.txpool.get_map().get(tx_hash) {
			Some(tx) => tx.tx.clone(),
			None => return Ok(None),
		};
		Ok(Some(tx))
	}

	fn coordinator_tx(&self) -> CommonResult<UnboundedSender<CoordinatorInMessage>> {
		Ok(self.coordinator.coordinator_tx())
	}
}
