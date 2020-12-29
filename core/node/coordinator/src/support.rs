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

use node_chain::{Chain, ChainOutMessage};
use primitives::errors::CommonResult;
use primitives::{BlockNumber, Hash, Header, Body, Transaction};

#[async_trait]
pub trait CoordinatorSupport {
	fn chain_rx(&self) -> Option<UnboundedReceiver<ChainOutMessage>>;
	fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>>;
	fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>>;
	fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>>;
	fn get_body(&self, block_hash: &Hash) -> CommonResult<Option<Body>>;
	fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>>;
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
}
