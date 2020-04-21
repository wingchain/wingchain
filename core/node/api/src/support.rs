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

use node_chain::Chain;
use primitives::errors::CommonResult;
use primitives::{Block, BlockNumber, Hash, Header, Transaction};

pub trait ApiSupport {
	fn get_best_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_executed_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	fn get_block(&self, block_hash: &Hash) -> CommonResult<Option<Block>>;
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
	fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>>;
}

pub struct DefaultApiSupport {
	chain: Arc<Chain>,
}

impl DefaultApiSupport {
	pub fn new(chain: Arc<Chain>) -> Self {
		Self { chain }
	}
}

impl ApiSupport for DefaultApiSupport {
	fn get_best_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_best_number()
	}
	fn get_executed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.chain.get_executed_number()
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
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.chain.get_transaction(tx_hash)
	}
	fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>> {
		self.chain.get_raw_transaction(tx_hash)
	}
}
