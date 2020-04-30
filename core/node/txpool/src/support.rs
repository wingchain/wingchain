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
use primitives::errors::CommonResult;
use primitives::{Hash, Transaction};

pub trait TxPoolSupport {
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash>;
	fn validate_transaction(&self, tx: &Transaction, witness_required: bool) -> CommonResult<()>;
}

impl TxPoolSupport for Chain {
	fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.hash_transaction(tx)
	}
	fn validate_transaction(&self, tx: &Transaction, witness_required: bool) -> CommonResult<()> {
		self.validate_transaction(tx, witness_required)
	}
}
