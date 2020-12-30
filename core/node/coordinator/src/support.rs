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
use futures::channel::mpsc::UnboundedReceiver;

use node_chain::{Chain, ChainCommitBlockParams, ChainOutMessage, CommitBlockResult};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::{
	Address, BlockNumber, Body, BuildBlockParams, CallResult, Hash, Header, Transaction,
};

#[async_trait]
pub trait CoordinatorSupport {
	fn chain_rx(&self) -> Option<UnboundedReceiver<ChainOutMessage>>;
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_execution_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	fn get_body(&self, block_hash: &Hash) -> CommonResult<Option<Body>>;
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	fn build_block(
		&self,
		build_block_params: BuildBlockParams,
	) -> CommonResult<ChainCommitBlockParams>;
	fn commit_block(
		&self,
		commit_block_params: ChainCommitBlockParams,
	) -> CommonResult<CommitBlockResult>;
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash>;
	fn validate_transaction(&self, tx: &Transaction, witness_required: bool) -> CommonResult<()>;
	fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>>;
}

pub struct DefaultCoordinatorSupport {
	chain: Arc<Chain>,
}

impl DefaultCoordinatorSupport {
	pub fn new(chain: Arc<Chain>) -> Self {
		Self { chain }
	}
}

impl CoordinatorSupport for DefaultCoordinatorSupport {
	fn chain_rx(&self) -> Option<UnboundedReceiver<ChainOutMessage>> {
		self.chain.message_rx()
	}
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
	fn get_body(&self, block_hash: &Hash) -> CommonResult<Option<Body>> {
		self.chain.get_body(block_hash)
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
	fn commit_block(
		&self,
		commit_block_params: ChainCommitBlockParams,
	) -> CommonResult<CommitBlockResult> {
		self.chain.commit_block(commit_block_params)
	}
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.chain.hash_transaction(tx)
	}
	fn validate_transaction(&self, tx: &Transaction, witness_required: bool) -> CommonResult<()> {
		self.chain.validate_transaction(tx, witness_required)
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
}
