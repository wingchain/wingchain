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

use node_chain::Chain;
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::CallResult;
use primitives::{Address, BlockNumber, Hash, Transaction};
use std::sync::Arc;

pub trait TxPoolSupport {
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash>;
	fn validate_transaction(
		&self,
		tx_hash: &Hash,
		tx: &Transaction,
		witness_required: bool,
	) -> CommonResult<()>;
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>>;
}

pub struct DefaultTxPoolSupport {
	chain: Arc<Chain>,
}

impl DefaultTxPoolSupport {
	pub fn new(chain: Arc<Chain>) -> Self {
		Self { chain }
	}
}

impl TxPoolSupport for DefaultTxPoolSupport {
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
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.chain.get_transaction(tx_hash)
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
