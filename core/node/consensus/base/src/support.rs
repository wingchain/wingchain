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

use std::collections::HashSet;
use std::sync::Arc;

use node_chain::{Basic, Chain, ChainCommitBlockParams, CurrentState, DBTransaction};
use node_txpool::support::DefaultTxPoolSupport;
use node_txpool::TxPool;
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::CallResult;
use primitives::{
	Address, BlockNumber, BuildBlockParams, Call, FullTransaction, Hash, Header, Proof, Transaction,
};

pub trait ConsensusSupport: Send + Sync + 'static {
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_execution_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	fn get_proof(&self, block_hash: &Hash) -> CommonResult<Option<Proof>>;
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	fn validate_transaction(
		&self,
		tx_hash: &Hash,
		tx: &Transaction,
		witness_required: bool,
	) -> CommonResult<()>;
	fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>>;
	fn is_meta_call(&self, call: &Call) -> CommonResult<bool>;
	fn build_block(
		&self,
		build_block_params: BuildBlockParams,
	) -> CommonResult<ChainCommitBlockParams>;
	fn commit_block(&self, commit_block_params: ChainCommitBlockParams) -> CommonResult<()>;
	fn get_basic(&self) -> CommonResult<Arc<Basic>>;
	fn get_current_state(&self) -> Arc<CurrentState>;
	fn get_consensus_data<T: Decode>(&self, key: &[u8]) -> CommonResult<Option<T>>;
	fn update_consensus_data<T: Encode>(
		&self,
		transaction: &mut DBTransaction,
		key: &[u8],
		value: T,
	) -> CommonResult<()>;
	fn commit_consensus_data(&self, transaction: DBTransaction) -> CommonResult<()>;
	fn txpool_get_transactions(&self) -> CommonResult<Vec<Arc<FullTransaction>>>;
	fn txpool_remove_transactions(&self, tx_hash_set: &HashSet<Hash>) -> CommonResult<()>;
}

pub struct DefaultConsensusSupport {
	chain: Arc<Chain>,
	txpool: Arc<TxPool<DefaultTxPoolSupport>>,
}

impl DefaultConsensusSupport {
	pub fn new(chain: Arc<Chain>, txpool: Arc<TxPool<DefaultTxPoolSupport>>) -> Self {
		Self { chain, txpool }
	}
}

impl ConsensusSupport for DefaultConsensusSupport {
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_confirmed_number()
	}
	fn get_execution_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_execution_number()
	}
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>> {
		self.chain.get_block_hash(number)
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
	fn validate_transaction(
		&self,
		tx_hash: &Hash,
		tx: &Transaction,
		witness_required: bool,
	) -> CommonResult<()> {
		self.chain
			.validate_transaction(tx_hash, tx, witness_required)
	}
	fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>> {
		self.chain
			.execute_call_with_block_number(block_number, sender, module, method, params)
	}
	fn is_meta_call(&self, call: &Call) -> CommonResult<bool> {
		self.chain.is_meta_call(call)
	}
	fn build_block(
		&self,
		build_block_params: BuildBlockParams,
	) -> CommonResult<ChainCommitBlockParams> {
		self.chain.build_block(build_block_params)
	}
	fn commit_block(&self, commit_block_params: ChainCommitBlockParams) -> CommonResult<()> {
		self.chain.commit_block(commit_block_params)
	}
	fn get_basic(&self) -> CommonResult<Arc<Basic>> {
		Ok(self.chain.get_basic())
	}
	fn get_current_state(&self) -> Arc<CurrentState> {
		self.chain.get_current_state()
	}
	fn get_consensus_data<T: Decode>(&self, key: &[u8]) -> CommonResult<Option<T>> {
		self.chain.get_consensus_data(key)
	}
	fn update_consensus_data<T: Encode>(
		&self,
		transaction: &mut DBTransaction,
		key: &[u8],
		value: T,
	) -> CommonResult<()> {
		self.chain.update_consensus_data(transaction, key, value)
	}
	fn commit_consensus_data(&self, transaction: DBTransaction) -> CommonResult<()> {
		self.chain.commit_consensus_data(transaction)
	}
	fn txpool_get_transactions(&self) -> CommonResult<Vec<Arc<FullTransaction>>> {
		let txs = (*self.txpool.get_queue().read()).clone();
		Ok(txs)
	}
	fn txpool_remove_transactions(&self, tx_hash_set: &HashSet<Hash>) -> CommonResult<()> {
		self.txpool.remove(tx_hash_set)
	}
}
