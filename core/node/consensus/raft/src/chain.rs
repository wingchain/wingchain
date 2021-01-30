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

use crate::state::{LogIndex, Term};
use crate::RaftStream;

const DB_KEY_LAST_LOG_INDEX: &[u8] = b"last_log_index";
const DB_KEY_LAST_LOG_TERM: &[u8] = b"last_log_term";
const DB_KEY_CURRENT_TERM: &[u8] = b"current_term";

/// methods for get/update chain data
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	pub fn chain_get_last_log_index(&self) -> CommonResult<LogIndex> {
		let value = self.support.get_consensus_data(DB_KEY_LAST_LOG_INDEX)?;
		let value = value.unwrap_or_default();
		Ok(value)
	}

	pub fn chain_get_last_log_term(&self) -> CommonResult<Term> {
		let value = self.support.get_consensus_data(DB_KEY_LAST_LOG_TERM)?;
		let value = value.unwrap_or_default();
		Ok(value)
	}

	pub fn chain_get_current_term(&self) -> CommonResult<Term> {
		let value = self.support.get_consensus_data(DB_KEY_CURRENT_TERM)?;
		let value = value.unwrap_or_default();
		Ok(value)
	}

	pub fn chain_init_update(&self) -> DBTransaction {
		DBTransaction::new()
	}

	pub fn chain_commit_update(&self, transaction: DBTransaction) -> CommonResult<()> {
		self.support.commit_consensus_data(transaction)
	}

	pub fn chain_update_last_log_index(
		&self,
		transaction: &mut DBTransaction,
		value: LogIndex,
	) -> CommonResult<()> {
		self.support
			.update_consensus_data(transaction, DB_KEY_LAST_LOG_INDEX, value)
	}

	pub fn chain_update_last_log_term(
		&self,
		transaction: &mut DBTransaction,
		value: Term,
	) -> CommonResult<()> {
		self.support
			.update_consensus_data(transaction, DB_KEY_LAST_LOG_TERM, value)
	}

	pub fn chain_update_current_term(
		&self,
		transaction: &mut DBTransaction,
		value: Term,
	) -> CommonResult<()> {
		self.support
			.update_consensus_data(transaction, DB_KEY_CURRENT_TERM, value)
	}

	pub fn chain_get_confirmed_number(&self) {}
}
