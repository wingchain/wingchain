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

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::HashImpl;
use node_db::DB;
use node_statedb::StateDB;
use primitives::{Block, Transaction};

use crate::errors;

pub struct Config {
	pub hash: Arc<HashImpl>,
	pub dsa: Arc<DsaImpl>,
	pub address: Arc<AddressImpl>,
}

pub struct Chain {
	db: Arc<DB>,
	meta_statedb: StateDB,
	statedb: StateDB,
}

impl Chain {
	pub fn new(db: Arc<DB>, config: Config) -> errors::Result<Self> {
		let meta_statedb = StateDB::new(db.clone(), node_db::columns::META_STATE, config.hash.clone())?;
		let statedb = StateDB::new(db.clone(), node_db::columns::STATE, config.hash)?;

		let chain = Chain {
			db,
			meta_statedb,
			statedb,
		};
		Ok(chain)
	}

	pub fn propose_block(&self, proposal: BlockProposal) -> Block {
		unimplemented!()
	}
}

pub struct BlockProposal {
	pub timestamp: u32,
	pub meta_txs: Vec<Transaction>,
	pub payload_txs: Vec<Transaction>,
}
