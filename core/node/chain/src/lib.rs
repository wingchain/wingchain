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

//! Chain to handle the db, statedb and executor

use std::path::PathBuf;
use std::sync::Arc;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::HashImpl;
use node_db::DBTransaction;
pub use node_executor::module;
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::CallResult;
use primitives::{
	Address, Block, BlockNumber, BuildBlockParams, Call, CommitBlockParams, CommitExecutionParams,
	Execution, Hash, Header, Nonce, OpaqueCallResult, Receipt, SecretKey, Transaction,
};

use crate::backend::Backend;
use crate::execute::{ExecuteQueue, ExecuteTask};

mod backend;
pub mod errors;
mod execute;
mod genesis;

pub type ChainCommitBlockParams = CommitBlockParams<DBTransaction>;
pub type ChainCommitExecutionParams = CommitExecutionParams<DBTransaction>;

pub struct ChainConfig {
	/// Home path
	pub home: PathBuf,
}

pub struct Chain {
	backend: Arc<Backend>,
	execute_queue: Arc<ExecuteQueue>,
}

pub struct Basic {
	pub hash: Arc<HashImpl>,
	pub dsa: Arc<DsaImpl>,
	pub address: Arc<AddressImpl>,
}

impl Chain {
	pub fn new(config: ChainConfig) -> CommonResult<Self> {
		let backend = Arc::new(Backend::new(config)?);

		let execute_queue = Arc::new(ExecuteQueue::new(backend.clone()));

		let chain = Self {
			backend,
			execute_queue,
		};

		Ok(chain)
	}

	/// Get the block hash by block number
	pub fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>> {
		self.backend.get_block_hash(number)
	}

	/// Get the header by block hash
	pub fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>> {
		self.backend.get_header(block_hash)
	}

	/// Get the block by block hash
	pub fn get_block(&self, block_hash: &Hash) -> CommonResult<Option<Block>> {
		self.backend.get_block(block_hash)
	}

	/// Get the execution by block hash
	pub fn get_execution(&self, block_hash: &Hash) -> CommonResult<Option<Execution>> {
		self.backend.get_execution(block_hash)
	}

	/// Get the transaction by transaction hash
	pub fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.backend.get_transaction(tx_hash)
	}

	/// Get the raw transaction (byte array) by transaction hash
	pub fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>> {
		self.backend.get_raw_transaction(tx_hash)
	}

	/// Get the receipt by transaction hash
	pub fn get_receipt(&self, tx_hash: &Hash) -> CommonResult<Option<Receipt>> {
		self.backend.get_receipt(tx_hash)
	}

	/// Determine if the given transaction is meta transaction
	pub fn is_meta_tx(&self, tx: &Transaction) -> CommonResult<bool> {
		self.backend.is_meta_tx(tx)
	}

	/// Get the hash of the given transaction
	pub fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.backend.hash_transaction(tx)
	}

	/// Validate transaction
	pub fn validate_transaction(
		&self,
		tx: &Transaction,
		witness_required: bool,
	) -> CommonResult<()> {
		self.backend.validate_transaction(tx, witness_required)
	}

	/// Build a transaction by witness, module, method and params
	pub fn build_transaction<P: Encode>(
		&self,
		witness: Option<(SecretKey, Nonce, BlockNumber)>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<Transaction> {
		self.backend
			.build_transaction(witness, module, method, params)
	}

	/// Build a block
	pub fn build_block(
		&self,
		build_block_params: BuildBlockParams,
	) -> CommonResult<ChainCommitBlockParams> {
		self.backend.build_block(build_block_params)
	}

	/// Commit a block
	/// this will persist the block into the db
	/// and insert a execute task into the execute queue
	pub async fn commit_block(
		&self,
		commit_block_params: ChainCommitBlockParams,
	) -> CommonResult<()> {
		let number = commit_block_params.header.number;
		let timestamp = commit_block_params.header.timestamp;
		let block_hash = commit_block_params.block_hash.clone();
		let parent_hash = commit_block_params.header.parent_hash.clone();
		let meta_state_root = commit_block_params.header.meta_state_root.clone();
		let payload_txs = commit_block_params.payload_txs.clone();

		self.backend.commit_block(commit_block_params)?;

		let execute_task = ExecuteTask {
			number,
			timestamp,
			block_hash,
			parent_hash,
			meta_state_root,
			payload_txs,
		};
		self.execute_queue.insert_task(execute_task).await?;
		Ok(())
	}

	/// Get the basic algorithms: das, hash and address
	pub fn get_basic(&self) -> Arc<Basic> {
		self.backend.get_basic()
	}

	/// Get the confirmed block number (namely best number or max height)
	pub fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.backend.get_confirmed_number()
	}

	/// Get the confirmed execution block number
	pub fn get_confirmed_executed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.backend.get_confirmed_executed_number()
	}

	/// Execute a call on a certain block specified by block hash
	/// this will not commit to the chain
	pub fn execute_call(
		&self,
		block_hash: &Hash,
		sender: Option<&Address>,
		call: &Call,
	) -> CommonResult<OpaqueCallResult> {
		self.backend.execute_call(block_hash, sender, call)
	}

	/// Execute a call on a certain block specified by block number
	/// this will not commit to the chain
	pub fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>> {
		self.backend
			.execute_call_with_block_number(block_number, sender, module, method, params)
	}
}
