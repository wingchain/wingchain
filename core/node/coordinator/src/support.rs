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

use async_trait::async_trait;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};

use node_chain::{Chain, ChainCommitBlockParams, ChainOutMessage, CurrentState};
use node_consensus::Consensus;
use node_consensus_base::support::DefaultConsensusSupport;
use node_consensus_base::{ConsensusInMessage, ConsensusOutMessage};
use node_txpool::support::DefaultTxPoolSupport;
use node_txpool::{TxPool, TxPoolOutMessage};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::{
	Address, BlockNumber, Body, BuildBlockParams, CallResult, FullTransaction, Hash, Header, Proof,
	Transaction,
};

#[async_trait]
pub trait CoordinatorSupport: Send + Sync + 'static {
	fn chain_rx(&self) -> Option<UnboundedReceiver<ChainOutMessage>>;
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	fn get_body(&self, block_hash: &Hash) -> CommonResult<Option<Body>>;
	fn get_proof(&self, block_hash: &Hash) -> CommonResult<Option<Proof>>;
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	fn build_block(
		&self,
		build_block_params: BuildBlockParams,
	) -> CommonResult<ChainCommitBlockParams>;
	fn commit_block(&self, commit_block_params: ChainCommitBlockParams) -> CommonResult<()>;
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash>;
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
	fn get_current_state(&self) -> Arc<CurrentState>;
	fn txpool_rx(&self) -> Option<UnboundedReceiver<TxPoolOutMessage>>;
	fn txpool_get_transactions(&self) -> CommonResult<Vec<Arc<FullTransaction>>>;
	fn txpool_get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Arc<FullTransaction>>>;
	fn txpool_insert_transaction(&self, tx: Transaction) -> CommonResult<()>;
	fn txpool_remove_transactions(&self, tx_hash_set: &HashSet<Hash>) -> CommonResult<()>;
	fn consensus_verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()>;
	fn consensus_tx(&self) -> UnboundedSender<ConsensusInMessage>;
	fn consensus_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>>;
}

pub struct DefaultCoordinatorSupport {
	chain: Arc<Chain>,
	txpool: Arc<TxPool<DefaultTxPoolSupport>>,
	consensus: Arc<Consensus<DefaultConsensusSupport>>,
}

impl DefaultCoordinatorSupport {
	pub fn new(
		chain: Arc<Chain>,
		txpool: Arc<TxPool<DefaultTxPoolSupport>>,
		consensus: Arc<Consensus<DefaultConsensusSupport>>,
	) -> Self {
		Self {
			chain,
			txpool,
			consensus,
		}
	}
}

#[async_trait]
impl CoordinatorSupport for DefaultCoordinatorSupport {
	fn chain_rx(&self) -> Option<UnboundedReceiver<ChainOutMessage>> {
		self.chain.message_rx()
	}
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>> {
		self.chain.get_block_hash(number)
	}
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>> {
		self.chain.get_header(block_hash)
	}
	fn get_body(&self, block_hash: &Hash) -> CommonResult<Option<Body>> {
		self.chain.get_body(block_hash)
	}
	fn get_proof(&self, block_hash: &Hash) -> CommonResult<Option<Proof>> {
		self.chain.get_proof(block_hash)
	}
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.chain.get_transaction(tx_hash)
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
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.chain.hash_transaction(tx)
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
	fn get_current_state(&self) -> Arc<CurrentState> {
		self.chain.get_current_state()
	}
	fn txpool_rx(&self) -> Option<UnboundedReceiver<TxPoolOutMessage>> {
		self.txpool.message_rx()
	}
	fn txpool_get_transactions(&self) -> CommonResult<Vec<Arc<FullTransaction>>> {
		let txs = (*self.txpool.get_queue().read()).clone();
		Ok(txs)
	}
	fn txpool_get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Arc<FullTransaction>>> {
		let tx = self.txpool.get_map().get(&tx_hash).as_deref().cloned();
		Ok(tx)
	}
	fn txpool_insert_transaction(&self, tx: Transaction) -> CommonResult<()> {
		self.txpool.insert(tx)
	}
	fn txpool_remove_transactions(&self, tx_hash_set: &HashSet<Hash>) -> CommonResult<()> {
		self.txpool.remove(tx_hash_set)
	}
	fn consensus_verify_proof(&self, header: &Header, proof: &Proof) -> CommonResult<()> {
		self.consensus.verify_proof(header, proof)
	}
	fn consensus_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		self.consensus.in_message_tx()
	}
	fn consensus_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		self.consensus.out_message_rx()
	}
}
