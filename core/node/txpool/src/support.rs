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

pub trait TxPoolSupport {
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash>;
	fn validate_transaction(&self, tx: &Transaction, witness_required: bool) -> CommonResult<()>;
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>>;
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

impl TxPoolSupport for Chain {
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.hash_transaction(tx)
	}
	fn validate_transaction(&self, tx: &Transaction, witness_required: bool) -> CommonResult<()> {
		self.validate_transaction(tx, witness_required)
	}
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.get_confirmed_number()
	}
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.get_transaction(tx_hash)
	}
	fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>> {
		self.execute_call_with_block_number(block_number, sender, module, method, params)
	}
}
