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

use node_chain::DBTransaction;
use node_consensus_base::support::ConsensusSupport;
use primitives::errors::CommonResult;
use primitives::{codec, Address};

use crate::proof::Proof;
use parking_lot::RwLock;
use std::collections::btree_map::Range;
use std::collections::BTreeMap;
use std::ops::RangeBounds;
use std::sync::Arc;

pub struct Storage<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
}

impl<S> Storage<S>
where
	S: ConsensusSupport,
{
	pub fn new(support: Arc<S>) -> CommonResult<Self> {
		let this = Self {
			support,
		};
		this.refresh()?;
		Ok(this)
	}

	pub fn refresh(&self) -> CommonResult<()> {
		unimplemented!()
	}

	fn commit_consensus_data<OP: Fn(&mut DBTransaction) -> CommonResult<()>>(
		&self,
		op: OP,
	) -> CommonResult<()> {
		let mut transaction = DBTransaction::new();
		op(&mut transaction)?;
		self.support.commit_consensus_data(transaction)
	}
}
