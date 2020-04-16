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
use codec::{Decode, Encode};
use log::info;
use toml::Value;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::{Hash as HashT, HashImpl};
use main_base::spec::Spec;
use main_base::SystemInitParams;
use node_db::{DBTransaction, DB};
use node_executor::{module, Context, Executor, ModuleEnum};
use node_statedb::{StateDB, TrieRoot};
use primitives::errors::CommonResult;
use primitives::{Block, BlockNumber, Body, DBKey, Executed, Hash, Header, Transaction};

pub mod errors;

pub struct ChainConfig {
	pub home: PathBuf,
}

pub struct Chain {
	db: Arc<DB>,
	config: ChainConfig,
	meta_statedb: Arc<StateDB>,
	payload_statedb: Arc<StateDB>,
	trie_root: Arc<TrieRoot>,
	executor: Executor,
	basic: Arc<Basic>,
}

pub struct Basic {
	hash: Arc<HashImpl>,
	#[allow(dead_code)]
	dsa: Arc<DsaImpl>,
	#[allow(dead_code)]
	address: Arc<AddressImpl>,
}

impl Chain {
	pub fn new(config: ChainConfig) -> CommonResult<Self> {
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
			node_db::columns::PAYLOAD_STATE,
			hash.clone(),
		)?);
		let trie_root = Arc::new(TrieRoot::new(hash.clone())?);

		let executor = Executor::new();

		let basic = Arc::new(Basic { hash, dsa, address });

		let chain = Self {
			db,
			config,
			meta_statedb,
			payload_statedb,
			trie_root,
			executor,
			basic,
		};

		info!("Initializing chain: genesis_inited: {}", genesis_inited);

		if !genesis_inited {
			chain.init_genesis()?;
		}

		Ok(chain)
	}

	#[allow(dead_code)]
	pub fn hash<D: Encode>(&self, data: &D) -> Hash {
		self.hash_slice(&data.encode())
	}

	pub fn hash_slice(&self, data: &[u8]) -> Hash {
		let mut out = vec![0u8; self.basic.hash.length().into()];
		self.basic.hash.hash(&mut out, data);
		Hash(out)
	}

	pub fn validate_tx(&self, tx: &Transaction) -> CommonResult<()> {
		self.executor.validate_tx(tx)
	}

	#[allow(dead_code)]
	pub fn get_best_number(&self) -> CommonResult<Option<BlockNumber>> {
		let best_number = self
			.db
			.get(node_db::columns::GLOBAL, node_db::global_key::BEST_NUMBER)?;

		let best_number = match best_number {
			Some(best_number) => {
				Decode::decode(&mut &best_number[..]).map_err(|e| errors::ErrorKind::Codec(e))?
			}
			None => None,
		};
		Ok(best_number)
	}

	fn get_spec(config: &ChainConfig) -> CommonResult<(bool, DB, Spec)> {
		let db_path = config.home.join(main_base::DATA).join(main_base::DB);
		let db = DB::open(&db_path)?;
		let genesis_inited = db
			.get(node_db::columns::GLOBAL, node_db::global_key::BEST_NUMBER)?
			.is_some();
		let spec = match genesis_inited {
			true => {
				let spec = db.get(node_db::columns::GLOBAL, node_db::global_key::SPEC)?;
				let spec = spec.ok_or(errors::ErrorKind::Spec("missing spec in db".to_string()))?;
				let spec: String = Decode::decode(&mut &spec[..])
					.map_err(|_| errors::ErrorKind::Spec("codec error".to_string()))?;
				spec
			}
			false => {
				let spec_path = config
					.home
					.join(main_base::CONFIG)
					.join(main_base::SPEC_FILE);
				let spec = fs::read_to_string(&spec_path).map_err(|_| {
					errors::ErrorKind::Spec(format!("failed to read spec file: {:?}", spec_path))
				})?;
				spec
			}
		};
		let spec = toml::from_str(&spec)
			.map_err(|e| errors::ErrorKind::Spec(format!("failed to parse spec file: {:?}", e)))?;

		Ok((genesis_inited, db, spec))
	}

	fn init_genesis(&self) -> CommonResult<()> {
		let spec_path = self
			.config
			.home
			.join(main_base::CONFIG)
			.join(main_base::SPEC_FILE);
		let spec_str = fs::read_to_string(&spec_path).map_err(|_| {
			errors::ErrorKind::Spec(format!("failed to read spec file: {:?}", spec_path))
		})?;
		let spec: Spec = toml::from_str(&spec_str)
			.map_err(|e| errors::ErrorKind::Spec(format!("failed to parse spec file: {:?}", e)))?;

		let tx = match spec.genesis.txs.get(0) {
			Some(tx) if tx.method == "system.init" => tx,
			_ => {
				return Err(errors::ErrorKind::Spec(format!(
					"invalid genesis txs: missing system.init"
				))
				.into());
			}
		};

		let param = match tx.params.get(0) {
			Some(Value::String(param)) => match serde_json::from_str::<SystemInitParams>(param) {
				Ok(param) => param,
				_ => {
					return Err(errors::ErrorKind::Spec(format!(
						"invalid genesis txs: invalid system.init params"
					))
					.into());
				}
			},
			_ => {
				return Err(errors::ErrorKind::Spec(format!(
					"invalid genesis txs: invalid system.init params"
				))
				.into());
			}
		};

		let chain_id = param.chain_id.clone();
		let time = DateTime::parse_from_rfc3339(&param.time).map_err(|_| {
			errors::ErrorKind::Spec(format!(
				"invalid genesis txs: invalid system.init param time: {:?}",
				param.time.clone()
			))
		})?;
		let timestamp = time.timestamp() as u32;

		let tx = Arc::new(
			self.executor
				.build_tx(
					ModuleEnum::System,
					module::system::MethodEnum::Init,
					module::system::InitParams {
						chain_id,
						timestamp,
					},
				)
				.expect("qed"),
		);
		let meta_txs = vec![tx];
		let payload_txs = vec![];
		let zero_hash = Hash(vec![0u8; self.basic.hash.length().into()]);

		let number = 0;
		let context = Context::new(
			number,
			timestamp,
			self.trie_root.clone(),
			self.meta_statedb.clone(),
			Hash(self.meta_statedb.default_root()),
			self.payload_statedb.clone(),
			Hash(self.payload_statedb.default_root()),
		)?;

		self.executor.execute_txs(&context, meta_txs)?;
		self.executor.execute_txs(&context, payload_txs)?;

		let (meta_state_root, meta_transaction) = context.get_meta_update()?;
		let (meta_txs_root, meta_txs) = context.get_meta_txs()?;

		let (payload_state_root, payload_transaction) = context.get_meta_update()?;
		let (payload_txs_root, payload_txs) = context.get_payload_txs()?;

		drop(context);

		// In common case, before reaching consensus and beginning to commit the new block, we should deref and clone Arc<Transaction>,
		// however, for genesis block, we're sure Arc<Transaction> is released, and we can use try_unwrap.
		let meta_txs = meta_txs
			.into_iter()
			.map(Arc::try_unwrap)
			.collect::<Result<Vec<_>, _>>()
			.expect("qed");
		let payload_txs = payload_txs
			.into_iter()
			.map(Arc::try_unwrap)
			.collect::<Result<Vec<_>, _>>()
			.expect("qed");

		let block = Block {
			header: Header {
				number,
				timestamp,
				parent_hash: zero_hash.clone(),
				meta_txs_root,
				meta_state_root,
				payload_txs_root,
				payload_executed_gap: 1,
				payload_executed_state_root: zero_hash,
			},
			body: Body {
				meta_txs,
				payload_txs,
			},
		};

		let mut transaction = DBTransaction::new();

		// commit block
		let header_encoded = Encode::encode(&block.header);
		let block_hash = self.hash_slice(&header_encoded);

		// 1. meta state
		transaction.extend(meta_transaction);

		// 2. header
		transaction.put_owned(
			node_db::columns::HEADER,
			DBKey::from_slice(&block_hash.0),
			header_encoded,
		);

		// 3. body
		transaction.put_owned(
			node_db::columns::META_TXS,
			DBKey::from_slice(&block_hash.0),
			Encode::encode(&block.body.meta_txs),
		);
		transaction.put_owned(
			node_db::columns::PAYLOAD_TXS,
			DBKey::from_slice(&block_hash.0),
			Encode::encode(&block.body.payload_txs),
		);

		// 4. block hash
		transaction.put_owned(
			node_db::columns::BLOCK_HASH,
			DBKey::from_slice(&Encode::encode(&number)),
			block_hash.0.clone(),
		);

		// 5. number
		transaction.put_owned(
			node_db::columns::GLOBAL,
			DBKey::from_slice(node_db::global_key::BEST_NUMBER),
			Encode::encode(&number),
		);

		// commit executed
		// 1. payload state
		transaction.extend(payload_transaction);

		// 2. executed
		let executed = Executed {
			payload_executed_state_root: payload_state_root,
		};
		transaction.put_owned(
			node_db::columns::EXECUTED,
			DBKey::from_slice(&block_hash.0),
			Encode::encode(&executed),
		);

		// commit spec
		transaction.put_owned(
			node_db::columns::GLOBAL,
			DBKey::from_slice(node_db::global_key::SPEC),
			Encode::encode(&spec_str),
		);

		self.db.write(transaction)?;

		info!("Genesis block inited: block hash: {:?}", block_hash);

		Ok(())
	}
}
