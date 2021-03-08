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

use crate::errors::ErrorKind;
use node_chain::ChainCommitBlockParams;
use node_consensus_base::support::ConsensusSupport;
use primitives::errors::{Catchable, CommonResult, Display};
use primitives::types::ExecutionGap;
use primitives::{BlockNumber, BuildBlockParams, FullTransaction, Hash, Header, Transaction};
use std::collections::HashSet;
use std::sync::Arc;

pub struct Verifier<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
}

#[derive(Debug, Display)]
pub enum VerifyError {
	#[display(fmt = "Duplicated")]
	Duplicated,
	/// Block is not the best
	#[display(fmt = "Not best")]
	NotBest,
	/// Invalid execution gap
	#[display(fmt = "Invalid execution gap")]
	InvalidExecutionGap,
	/// Should wait executing
	#[display(fmt = "Should wait")]
	ShouldWait,
	/// Invalid header
	#[display(fmt = "Invalid header: {}", _0)]
	InvalidHeader(String),
	/// Transaction duplicated
	#[display(fmt = "Duplicated tx: {}", _0)]
	DuplicatedTx(String),
	/// Transaction invalid
	#[display(fmt = "Invalid tx: {}", _0)]
	InvalidTx(node_chain::errors::ValidateTxError),
}

impl<S> Verifier<S>
where
	S: ConsensusSupport,
{
	pub fn new(support: Arc<S>) -> CommonResult<Self> {
		let verifier = Self { support };
		Ok(verifier)
	}
}

impl<S> Verifier<S>
where
	S: ConsensusSupport,
{

}
