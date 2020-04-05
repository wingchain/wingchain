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
use std::str::FromStr;
use std::sync::Arc;

use chrono::DateTime;
use parity_codec::{Decode, Encode};
use toml::Value;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::{Hash as HashT, HashImpl};
use main_base::spec::Spec;
use main_base::SystemInitParams;
use node_db::DB;
use node_executor::{Executor, module, ModuleEnum};
use node_statedb::{StateDB, TrieRoot};
use primitives::{Block, BlockNumber, Body, Hash, Header};
// use primitives::traits::Module;

use crate::errors;

pub struct Config {
	pub home: PathBuf,
}

pub struct Chain {
	db: Arc<DB>,
	config: Config,
	meta_statedb: Arc<StateDB>,
	payload_statedb: Arc<StateDB>,
	trie_root: Arc<TrieRoot>,
	basic: Arc<Basic>,
}

pub struct Basic {
	hash: Arc<HashImpl>,
	dsa: Arc<DsaImpl>,
	address: Arc<AddressImpl>,
}

impl Chain {
	pub fn new(config: Config) -> errors::Result<Self> {
		let (genesis_inited, db, spec) = Self::get_spec(&config)?;

		let db = Arc::new(db);
		let hash = Arc::new(HashImpl::from_str(&spec.basic.hash)?);
		let dsa = Arc::new(DsaImpl::from_str(&spec.basic.dsa)?);
		let address = Arc::new(AddressImpl::from_str(&spec.basic.address)?);

		let meta_statedb = Arc::new(StateDB::new(
			db.clone(),
			node_db::columns::META_STATE,
			hash.clone(),
		)?);
		let payload_statedb = Arc::new(StateDB::new(
			db.clone(),
			node_db::columns::STATE,
			hash.clone(),
		)?);
		let trie_root = Arc::new(TrieRoot::new(hash.clone())?);

		let basic = Arc::new(Basic {
			hash,
			dsa,
			address,
		});

		let chain = Self {
			db,
			config,
			meta_statedb,
			payload_statedb,
			trie_root,
			basic,
		};

		if !genesis_inited {
			chain.init_genesis()?;
		}

		Ok(chain)
	}

	fn get_spec(config: &Config) -> errors::Result<(bool, DB, Spec)> {
		let db_path = config.home.join(main_base::DATA).join(main_base::DB);
		let db = DB::open(&db_path)?;
		let genesis_inited = db.get(node_db::columns::GLOBAL, node_db::global_key::BEST_NUMBER)?.is_some();
		let spec = match genesis_inited {
			true => {
				let spec = db.get(node_db::columns::GLOBAL, node_db::global_key::SPEC)?;
				let spec = spec.and_then(|x| Decode::decode(&mut &x[..]));
				let spec: String = spec.ok_or(errors::ErrorKind::DBIntegrityLess("miss spec".to_string()))?;
				spec
			}
			false => {
				let spec = fs::read_to_string(config.home
					.join(main_base::CONFIG)
					.join(main_base::SPEC_FILE))?;
				spec
			}
		};
		let spec = toml::from_str(&spec)?;

		Ok((genesis_inited, db, spec))
	}

	fn init_genesis(&self) -> errors::Result<()> {
		let spec_str = fs::read_to_string(self.config.home
											  .join(main_base::CONFIG)
											  .join(main_base::SPEC_FILE),
		)?;
		let spec: Spec = toml::from_str(&spec_str)?;

		let tx = match spec.genesis.txs.get(0) {
			Some(tx) if tx.method == "system.init" => tx,
			_ => return Err(errors::ErrorKind::InvalidSpec.into()),
		};

		let param = match tx.params.get(0) {
			Some(Value::String(param)) => match serde_json::from_str::<SystemInitParams>(param) {
				Ok(param) => param,
				_ => return Err(errors::ErrorKind::InvalidSpec.into()),
			},
			_ => return Err(errors::ErrorKind::InvalidSpec.into()),
		};

		let chain_id = param.chain_id;
		let time = DateTime::parse_from_rfc3339(&param.time)
			.map_err(|_| errors::ErrorKind::InvalidSpec)?;
		let timestamp = time.timestamp() as u32;

		let init_params = module::system::InitParams {
			chain_id,
			timestamp,
		};

		let tx = Executor::build_tx(ModuleEnum::System, module::system::MethodEnum::Init, init_params).expect("qed");
		let meta_txs = vec![tx];
		let payload_txs = vec![];
		let zero_hash = Hash(vec![0u8; self.basic.hash.length().into()]);

		let meta_txs_root = Hash(self.trie_root.calc_ordered_trie_root(meta_txs.iter().map(Encode::encode)));
		let payload_txs_root = Hash(self.trie_root.calc_ordered_trie_root(payload_txs.iter().map(Encode::encode)));

		println!("meta_txs_root: {:?}", meta_txs_root);
		println!("payload_txs_root: {:?}", payload_txs_root);

		let block = Block {
			header: Header {
				number: 0,
				timestamp,
				parent_hash: zero_hash.clone(),
				meta_txs_root,
				meta_state_root: zero_hash.clone(),
				payload_txs_root,
				payload_executed_gap: 1,
				payload_executed_state_root: zero_hash,
			},
			body: Body {
				meta_txs,
				payload_txs,
			},
		};

		Ok(())
	}

	fn get_best_number(&self) -> errors::Result<Option<BlockNumber>> {
		let best_number = self
			.db
			.get(node_db::columns::GLOBAL, node_db::global_key::BEST_NUMBER)?
			.and_then(|x| Decode::decode(&mut &x[..]));
		Ok(best_number)
	}
}
