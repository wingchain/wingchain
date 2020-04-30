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

use log::info;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::{Hash as HashT, HashImpl};
use main_base::spec::Spec;
use node_db::{DBTransaction, DB};
pub use node_executor::module;
use node_executor::{Context, ContextEnv, ContextEssence, Executor};
use node_statedb::{StateDB, TrieRoot};
use primitives::codec::Encode;
use primitives::errors::CommonResult;
use primitives::{
	codec, Block, BlockNumber, Body, DBKey, Executed, FullBlock, Hash, Header, Nonce, SecretKey,
	Transaction, TransactionForHash,
};

use crate::genesis::{build_genesis, GenesisInfo};

pub mod errors;
mod genesis;

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
	pub hash: Arc<HashImpl>,
	pub dsa: Arc<DsaImpl>,
	pub address: Arc<AddressImpl>,
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

		let executor = Executor::new(dsa.clone(), address.clone());

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

	pub fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		let transaction_for_hash = TransactionForHash::new(tx);
		self.hash(&transaction_for_hash)
	}

	pub fn hash<D: Encode>(&self, data: &D) -> CommonResult<Hash> {
		let encoded = codec::encode(data)?;
		Ok(self.hash_slice(&encoded))
	}

	pub fn validate_transaction(
		&self,
		tx: &Transaction,
		witness_required: bool,
	) -> CommonResult<()> {
		self.executor.validate_tx(tx, witness_required)
	}

	pub fn build_transaction<P: Encode>(
		&self,
		witness: Option<(SecretKey, Nonce, BlockNumber)>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<Transaction> {
		self.executor.build_tx(witness, module, method, params)
	}

	pub fn get_basic(&self) -> Arc<Basic> {
		self.basic.clone()
	}

	pub fn get_best_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.db.get_with(
			node_db::columns::GLOBAL,
			node_db::global_key::BEST_NUMBER,
			|x| codec::decode(&x[..]),
		)
	}

	pub fn get_executed_number(&self) -> CommonResult<Option<BlockNumber>> {
		let best_number = match self.get_best_number()? {
			Some(best_number) => best_number,
			None => return Ok(None),
		};

		let block_hash = self
			.get_block_hash(&best_number)?
			.ok_or(errors::ErrorKind::Data(format!(
				"missing block hash: number: {}",
				best_number
			)))?;

		let header = self
			.get_header(&block_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"missing header: block_hash: {:?}",
				block_hash
			)))?;

		let executed_number = match best_number {
			0 => None,
			_ => Some(best_number - header.payload_executed_gap as u32),
		};

		Ok(executed_number)
	}

	pub fn get_block_hash(&self, number: &BlockNumber) -> CommonResult<Option<Hash>> {
		self.db.get_with(
			node_db::columns::BLOCK_HASH,
			&DBKey::from_slice(&codec::encode(&number)?),
			|x| codec::decode(&x[..]),
		)
	}

	pub fn get_header(&self, block_hash: &Hash) -> CommonResult<Option<Header>> {
		self.db.get_with(
			node_db::columns::HEADER,
			&DBKey::from_slice(&block_hash.0),
			|x| codec::decode(&x[..]),
		)
	}

	pub fn get_block(&self, block_hash: &Hash) -> CommonResult<Option<Block>> {
		let header = match self.get_header(block_hash)? {
			Some(header) => header,
			None => return Ok(None),
		};
		let meta_txs: Vec<Hash> = self
			.db
			.get_with(
				node_db::columns::META_TXS,
				&DBKey::from_slice(&block_hash.0),
				|x| codec::decode(&x[..]),
			)?
			.ok_or(errors::ErrorKind::Data(format!(
				"block missing meta_txs: block_hash: {:?}",
				block_hash
			)))?;

		let payload_txs: Vec<Hash> = self
			.db
			.get_with(
				node_db::columns::PAYLOAD_TXS,
				&DBKey::from_slice(&block_hash.0),
				|x| codec::decode(&x[..]),
			)?
			.ok_or(errors::ErrorKind::Data(format!(
				"block missing meta_txs: block_hash: {:?}",
				block_hash
			)))?;

		Ok(Some(Block {
			header,
			body: Body {
				meta_txs,
				payload_txs,
			},
		}))
	}

	pub fn get_executed(&self, block_hash: &Hash) -> CommonResult<Option<Executed>> {
		self.db.get_with(
			node_db::columns::EXECUTED,
			&DBKey::from_slice(&block_hash.0),
			|x| codec::decode(&x[..]),
		)
	}

	pub fn get_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Transaction>> {
		self.db
			.get_with(node_db::columns::TX, &DBKey::from_slice(&tx_hash.0), |x| {
				codec::decode(&x[..])
			})
	}

	pub fn get_raw_transaction(&self, tx_hash: &Hash) -> CommonResult<Option<Vec<u8>>> {
		self.db
			.get(node_db::columns::TX, &DBKey::from_slice(&tx_hash.0))
	}

	fn hash_slice(&self, data: &[u8]) -> Hash {
		let mut out = vec![0u8; self.basic.hash.length().into()];
		self.basic.hash.hash(&mut out, data);
		Hash(out)
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
				let spec: String = codec::decode(&mut &spec[..])
					.map_err(|_| errors::ErrorKind::Spec("serde error".to_string()))?;
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

		let GenesisInfo {
			meta_txs,
			payload_txs,
			timestamp,
		} = build_genesis(&spec, &self.executor)?;

		let zero_hash = Hash(vec![0u8; self.basic.hash.length().into()]);

		let number = 0;
		let env = ContextEnv { number, timestamp };

		let context_essence = ContextEssence::new(
			env,
			self.trie_root.clone(),
			self.meta_statedb.clone(),
			Hash(self.meta_statedb.default_root()),
			self.payload_statedb.clone(),
			Hash(self.payload_statedb.default_root()),
		)?;

		let context = Context::new(&context_essence)?;

		self.executor.execute_txs(&context, meta_txs)?;
		self.executor.execute_txs(&context, payload_txs)?;

		let (meta_state_root, meta_transaction) = context.get_meta_update()?;
		let (meta_txs_root, meta_txs) = context.get_meta_txs()?;

		let (payload_state_root, payload_transaction) = context.get_payload_update()?;
		let (payload_txs_root, payload_txs) = context.get_payload_txs()?;

		drop(context);

		// In common case, before reaching consensus and beginning to commit the new block, we should deref and clone Arc<Transaction>,
		// however, for genesis block, we're sure Arc<Transaction> is released, and we can use try_unwrap.
		let meta_txs = meta_txs
			.into_iter()
			.map(Arc::try_unwrap)
			.collect::<Result<Vec<Transaction>, _>>()
			.expect("qed");
		let payload_txs = payload_txs
			.into_iter()
			.map(Arc::try_unwrap)
			.collect::<Result<Vec<Transaction>, _>>()
			.expect("qed");

		let meta_txs = meta_txs
			.into_iter()
			.map(|x| self.hash_transaction(&x).map(|hash| (hash, x)))
			.collect::<CommonResult<Vec<_>>>()?;

		let payload_txs = payload_txs
			.into_iter()
			.map(|x| self.hash_transaction(&x).map(|hash| (hash, x)))
			.collect::<CommonResult<Vec<_>>>()?;

		let meta_tx_hashes = meta_txs
			.iter()
			.map(|(tx_hash, _)| tx_hash.clone())
			.collect::<Vec<_>>();
		let payload_tx_hashes = payload_txs
			.iter()
			.map(|(tx_hash, _)| tx_hash.clone())
			.collect::<Vec<_>>();

		let header = Header {
			number,
			timestamp,
			parent_hash: zero_hash.clone(),
			meta_txs_root,
			meta_state_root,
			payload_txs_root,
			payload_executed_gap: 1,
			payload_executed_state_root: zero_hash,
		};

		let block_hash = self.hash(&header)?;

		let txs = meta_txs
			.into_iter()
			.chain(payload_txs.into_iter())
			.collect::<Vec<_>>();

		let block = FullBlock {
			number,
			block_hash,
			header,
			body: Body {
				meta_txs: meta_tx_hashes,
				payload_txs: payload_tx_hashes,
			},
			txs,
		};

		let executed = Executed {
			payload_executed_state_root: payload_state_root,
		};

		let mut transaction = DBTransaction::new();

		commit_block(&mut transaction, &block, meta_transaction)?;

		commit_executed(
			&mut transaction,
			&block.block_hash,
			&executed,
			payload_transaction,
		)?;

		commit_spec(&mut transaction, &spec_str)?;

		self.db.write(transaction)?;

		info!("Genesis block inited: block hash: {:?}", block.block_hash);

		Ok(())
	}
}

fn commit_block(
	transaction: &mut DBTransaction,
	block: &FullBlock,
	meta_transaction: DBTransaction,
) -> CommonResult<()> {
	// 1. meta state
	transaction.extend(meta_transaction);

	let block_hash = &block.block_hash;

	// 2. header
	transaction.put_owned(
		node_db::columns::HEADER,
		DBKey::from_slice(&block_hash.0),
		codec::encode(&block.header)?,
	);

	// 3. body
	transaction.put_owned(
		node_db::columns::META_TXS,
		DBKey::from_slice(&block_hash.0),
		codec::encode(&block.body.meta_txs)?,
	);
	transaction.put_owned(
		node_db::columns::PAYLOAD_TXS,
		DBKey::from_slice(&block_hash.0),
		codec::encode(&block.body.payload_txs)?,
	);

	// 4. txs
	for (tx_hash, tx) in &block.txs {
		transaction.put_owned(
			node_db::columns::TX,
			DBKey::from_slice(&tx_hash.0),
			codec::encode(&tx)?,
		);
	}

	// 5. block hash
	transaction.put_owned(
		node_db::columns::BLOCK_HASH,
		DBKey::from_slice(&codec::encode(&block.number)?),
		codec::encode(&block.block_hash)?,
	);

	// 6. number
	transaction.put_owned(
		node_db::columns::GLOBAL,
		DBKey::from_slice(node_db::global_key::BEST_NUMBER),
		codec::encode(&block.number)?,
	);
	Ok(())
}

fn commit_executed(
	transaction: &mut DBTransaction,
	block_hash: &Hash,
	executed: &Executed,
	payload_transaction: DBTransaction,
) -> CommonResult<()> {
	// 1. payload state
	transaction.extend(payload_transaction);

	// 2. executed
	transaction.put_owned(
		node_db::columns::EXECUTED,
		DBKey::from_slice(&block_hash.0),
		codec::encode(&executed)?,
	);
	Ok(())
}

fn commit_spec(transaction: &mut DBTransaction, spec_str: &str) -> CommonResult<()> {
	transaction.put_owned(
		node_db::columns::GLOBAL,
		DBKey::from_slice(node_db::global_key::SPEC),
		codec::encode(&spec_str)?,
	);
	Ok(())
}
