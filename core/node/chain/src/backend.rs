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
use std::str::FromStr;
use std::sync::Arc;

use log::{debug, info};

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::HashImpl;
use main_base::spec::Spec;
use node_db::{DBTransaction, DB};
use node_executor::{Context, ContextEssence, Executor};
use node_executor_primitives::ContextEnv;
use node_statedb::{StateDB, TrieRoot};
use primitives::codec::{self, Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::CallResult;
use primitives::{
	Address, Block, BlockNumber, Body, BuildBlockParams, BuildExecuteParams, Call, DBKey, Executed,
	Hash, Header, Nonce, Receipt, SecretKey, Transaction, TransactionResult,
};

use crate::genesis::build_genesis;
use crate::{errors, Basic, ChainCommitBlockParams, ChainCommitExecuteParams, ChainConfig};

pub struct Backend {
	db: Arc<DB>,
	config: ChainConfig,
	meta_statedb: Arc<StateDB>,
	payload_statedb: Arc<StateDB>,
	trie_root: Arc<TrieRoot>,
	executor: Executor,
	basic: Arc<Basic>,
}

impl Backend {
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

		let executor = Executor::new(hash.clone(), dsa.clone(), address.clone());

		let basic = Arc::new(Basic { hash, dsa, address });

		let backend = Self {
			db,
			config,
			meta_statedb,
			payload_statedb,
			trie_root,
			executor,
			basic,
		};

		info!("Initializing backend: genesis_inited: {}", genesis_inited);

		if !genesis_inited {
			backend.init_genesis()?;
		}

		Ok(backend)
	}

	pub fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		self.executor.hash_transaction(tx)
	}

	pub fn hash<D: Encode>(&self, data: &D) -> CommonResult<Hash> {
		self.executor.hash(data)
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

	pub fn get_confirmed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.db.get_with(
			node_db::columns::GLOBAL,
			node_db::global_key::CONFIRMED_NUMBER,
			|x| codec::decode(&x[..]),
		)
	}

	pub fn get_executed_number(&self) -> CommonResult<Option<BlockNumber>> {
		self.db.get_with(
			node_db::columns::GLOBAL,
			node_db::global_key::EXECUTED_NUMBER,
			|x| codec::decode(&x[..]),
		)
	}

	pub fn get_confirmed_executed_number(&self) -> CommonResult<Option<BlockNumber>> {
		let confirmed_number = match self.get_confirmed_number()? {
			Some(confirmed_number) => confirmed_number,
			None => return Ok(None),
		};

		let block_hash = self
			.get_block_hash(&confirmed_number)?
			.ok_or(errors::ErrorKind::Data(format!(
				"missing block hash: number: {}",
				confirmed_number
			)))?;

		let header = self
			.get_header(&block_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"missing header: block_hash: {:?}",
				block_hash
			)))?;

		let confirmed_executed_number = match confirmed_number {
			0 => None,
			_ => Some(confirmed_number - header.payload_executed_gap as u64),
		};

		Ok(confirmed_executed_number)
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

	pub fn get_receipt(&self, tx_hash: &Hash) -> CommonResult<Option<Receipt>> {
		self.db.get_with(
			node_db::columns::RECEIPT,
			&DBKey::from_slice(&tx_hash.0),
			|x| codec::decode(&x[..]),
		)
	}

	pub fn execute_call(
		&self,
		block_hash: &Hash,
		sender: Option<&Address>,
		call: &Call,
	) -> CommonResult<TransactionResult> {
		let header = match self.get_header(block_hash)? {
			Some(header) => header,
			None => {
				return Err(
					errors::ErrorKind::Data(format!("unknown block hash: {}", block_hash)).into(),
				);
			}
		};

		let executed = match self.get_executed(block_hash)? {
			Some(executed) => executed,
			None => {
				return Err(errors::ErrorKind::Data(format!(
					"not executed block hash: {}",
					block_hash
				))
				.into());
			}
		};

		let number = header.number;
		let timestamp = header.timestamp;

		let meta_state_root = header.meta_state_root;
		let payload_state_root = executed.payload_executed_state_root;

		let env = ContextEnv { number, timestamp };

		let context_essence = ContextEssence::new(
			env,
			self.trie_root.clone(),
			self.meta_statedb.clone(),
			meta_state_root,
			self.payload_statedb.clone(),
			payload_state_root,
		)?;

		let context = Context::new(&context_essence)?;

		self.executor.execute_call(&context, sender, call)
	}

	pub fn execute_call_with_block_number<P: Encode, R: Decode>(
		&self,
		block_number: &BlockNumber,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>> {
		let block_hash = self
			.get_block_hash(block_number)?
			.ok_or(errors::ErrorKind::Data(format!(
				"unknown block number: {}",
				block_number
			)))?;
		self.execute_call_with_block_hash(&block_hash, sender, module, method, params)
	}

	pub fn execute_call_with_block_hash<P: Encode, R: Decode>(
		&self,
		block_hash: &Hash,
		sender: Option<&Address>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<CallResult<R>> {
		let call = self.build_transaction(None, module, method, params)?.call;
		let result = self.execute_call(&block_hash, sender, &call)?;
		let result: CallResult<R> = match result {
			Ok(result) => Ok(codec::decode(&result)?),
			Err(e) => Err(e),
		};
		Ok(result)
	}

	pub fn is_meta_tx(&self, tx: &Transaction) -> CommonResult<bool> {
		self.executor.is_meta_tx(tx)
	}

	pub fn build_block(
		&self,
		build_block_params: BuildBlockParams,
	) -> CommonResult<ChainCommitBlockParams> {
		let number = build_block_params.number;
		let timestamp = build_block_params.timestamp;
		let parent_number = number - 1;
		let parent_hash = match self.get_block_hash(&parent_number)? {
			Some(parent_hash) => parent_hash,
			None => {
				return Err(errors::ErrorKind::Data(format!(
					"invalid block number: {}",
					parent_number
				))
				.into());
			}
		};
		let env = ContextEnv { number, timestamp };

		let parent_header = match self.get_header(&parent_hash)? {
			Some(header) => header,
			None => {
				return Err(errors::ErrorKind::Data(format!(
					"invalid block hash: {}",
					parent_hash
				))
				.into());
			}
		};
		let meta_state_root = parent_header.meta_state_root.clone();

		let executed_number = match self.get_executed_number()? {
			Some(actual_executed_number) => actual_executed_number,
			None => {
				return Err(errors::ErrorKind::Data(format!("executed number not found")).into())
			}
		};
		let executed_block_hash = match self.get_block_hash(&executed_number)? {
			Some(executed_block_hash) => executed_block_hash,
			None => {
				return Err(errors::ErrorKind::Data(format!(
					"invalid block number: {}",
					executed_number
				))
				.into());
			}
		};
		let executed = match self.get_executed(&executed_block_hash)? {
			Some(actual_executed) => actual_executed,
			None => {
				return Err(errors::ErrorKind::Data(format!(
					"executed not found, block_hash: {}",
					executed_block_hash
				))
				.into())
			}
		};

		let block_executed_gap = (number - executed_number) as i8;
		let block_executed = executed;

		let context_essence = ContextEssence::new(
			env,
			self.trie_root.clone(),
			self.meta_statedb.clone(),
			meta_state_root,
			self.payload_statedb.clone(),
			block_executed.payload_executed_state_root.clone(),
		)?;

		let context = Context::new(&context_essence)?;

		self.executor
			.execute_txs(&context, build_block_params.meta_txs)?;

		let (meta_state_root, meta_transaction) = context.get_meta_update()?;
		let (meta_txs_root, meta_txs) = context.get_meta_txs()?;
		let (meta_receipts_root, meta_receipts) = context.get_meta_receipts()?;

		let payload_txs_root = context.get_txs_root(&build_block_params.payload_txs)?;
		let payload_txs = build_block_params.payload_txs;

		let meta_tx_hashes = meta_txs
			.iter()
			.map(|x| x.tx_hash.clone())
			.collect::<Vec<_>>();
		let payload_tx_hashes = payload_txs
			.iter()
			.map(|x| x.tx_hash.clone())
			.collect::<Vec<_>>();

		let header = Header {
			number,
			timestamp,
			parent_hash,
			meta_txs_root,
			meta_state_root,
			meta_receipts_root,
			payload_txs_root,
			payload_executed_gap: block_executed_gap,
			payload_executed_state_root: block_executed.payload_executed_state_root,
			payload_executed_receipts_root: block_executed.payload_executed_receipts_root,
		};

		let block_hash = self.hash(&header)?;

		let block = ChainCommitBlockParams {
			block_hash,
			header,
			body: Body {
				meta_txs: meta_tx_hashes,
				payload_txs: payload_tx_hashes,
			},
			meta_txs,
			meta_receipts,
			payload_txs,
			meta_transaction,
		};

		Ok(block)
	}

	pub fn commit_block(&self, commit_block_params: ChainCommitBlockParams) -> CommonResult<()> {
		debug!("Commit block params: {:?}", commit_block_params);

		let mut transaction = DBTransaction::new();

		let number = commit_block_params.header.number;
		let block_hash = commit_block_params.block_hash.clone();

		commit_block(&mut transaction, commit_block_params)?;

		self.db.write(transaction)?;

		info!(
			"Block committed: block number: {}, block hash: {:?}",
			number, block_hash
		);

		Ok(())
	}

	pub fn build_execute(
		&self,
		build_execute_params: BuildExecuteParams,
	) -> CommonResult<ChainCommitExecuteParams> {
		let number = build_execute_params.number;
		let timestamp = build_execute_params.timestamp;
		let block_hash = build_execute_params.block_hash;
		let meta_state_root = build_execute_params.meta_state_root;
		let payload_state_root = build_execute_params.payload_state_root;
		let payload_txs = build_execute_params.payload_txs;
		let env = ContextEnv { number, timestamp };

		let context_essence = ContextEssence::new(
			env,
			self.trie_root.clone(),
			self.meta_statedb.clone(),
			meta_state_root,
			self.payload_statedb.clone(),
			payload_state_root,
		)?;

		let context = Context::new(&context_essence)?;

		self.executor.execute_txs(&context, payload_txs)?;

		let (payload_state_root, payload_transaction) = context.get_payload_update()?;
		let (payload_receipts_root, payload_receipts) = context.get_payload_receipts()?;

		let commit_execute_params = ChainCommitExecuteParams {
			block_hash,
			number,
			executed: Executed {
				payload_executed_state_root: payload_state_root,
				payload_executed_receipts_root: payload_receipts_root,
			},
			payload_receipts,
			payload_transaction,
		};

		Ok(commit_execute_params)
	}

	pub fn commit_execute(
		&self,
		commit_execute_params: ChainCommitExecuteParams,
	) -> CommonResult<()> {
		debug!("Commit execute params: {:?}", commit_execute_params);

		let mut transaction = DBTransaction::new();

		let number = commit_execute_params.number;
		let block_hash = commit_execute_params.block_hash.clone();

		commit_executed(&mut transaction, commit_execute_params)?;

		self.db.write(transaction)?;

		info!(
			"Block executed: block number: {}, block hash: {:?}",
			number, block_hash
		);

		Ok(())
	}

	fn get_spec(config: &ChainConfig) -> CommonResult<(bool, DB, Spec)> {
		let db_path = config.home.join(main_base::DATA).join(main_base::DB);
		let db = DB::open(&db_path)?;
		let genesis_inited = db
			.get(
				node_db::columns::GLOBAL,
				node_db::global_key::CONFIRMED_NUMBER,
			)?
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

		let BuildBlockParams {
			number,
			timestamp,
			meta_txs,
			payload_txs,
		} = build_genesis(&spec, &self.executor)?;

		let zero_hash = self.executor.default_hash();

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
		let (meta_receipts_root, meta_receipts) = context.get_meta_receipts()?;

		let (payload_state_root, payload_transaction) = context.get_payload_update()?;
		let (payload_txs_root, payload_txs) = context.get_payload_txs()?;
		let (payload_receipts_root, payload_receipts) = context.get_payload_receipts()?;

		let meta_tx_hashes = meta_txs
			.iter()
			.map(|x| x.tx_hash.clone())
			.collect::<Vec<_>>();
		let payload_tx_hashes = payload_txs
			.iter()
			.map(|x| x.tx_hash.clone())
			.collect::<Vec<_>>();

		let header = Header {
			number,
			timestamp,
			parent_hash: zero_hash.clone(),
			meta_txs_root,
			meta_state_root,
			meta_receipts_root,
			payload_txs_root,
			payload_executed_gap: 1,
			payload_executed_state_root: zero_hash.clone(),
			payload_executed_receipts_root: zero_hash,
		};

		let block_hash = self.hash(&header)?;

		let commit_block_params = ChainCommitBlockParams {
			block_hash: block_hash.clone(),
			header,
			body: Body {
				meta_txs: meta_tx_hashes,
				payload_txs: payload_tx_hashes,
			},
			meta_txs,
			meta_receipts,
			payload_txs,
			meta_transaction,
		};

		let executed = Executed {
			payload_executed_state_root: payload_state_root,
			payload_executed_receipts_root: payload_receipts_root,
		};

		let mut transaction = DBTransaction::new();

		commit_block(&mut transaction, commit_block_params)?;

		let commit_execute_params = ChainCommitExecuteParams {
			block_hash: block_hash.clone(),
			number,
			executed,
			payload_receipts,
			payload_transaction,
		};

		commit_executed(&mut transaction, commit_execute_params)?;

		commit_spec(&mut transaction, &spec_str)?;

		self.db.write(transaction)?;

		info!("Genesis block inited: block hash: {:?}", block_hash);

		Ok(())
	}
}

fn commit_block(
	transaction: &mut DBTransaction,
	commit_block_params: ChainCommitBlockParams,
) -> CommonResult<()> {
	// 1. meta state
	transaction.extend(commit_block_params.meta_transaction);

	let block_hash = &commit_block_params.block_hash;

	// 2. header
	transaction.put_owned(
		node_db::columns::HEADER,
		DBKey::from_slice(&block_hash.0),
		codec::encode(&commit_block_params.header)?,
	);

	// 3. body
	transaction.put_owned(
		node_db::columns::META_TXS,
		DBKey::from_slice(&block_hash.0),
		codec::encode(&commit_block_params.body.meta_txs)?,
	);
	transaction.put_owned(
		node_db::columns::PAYLOAD_TXS,
		DBKey::from_slice(&block_hash.0),
		codec::encode(&commit_block_params.body.payload_txs)?,
	);

	// 4. txs
	for tx in &commit_block_params.meta_txs {
		let tx_hash = &tx.tx_hash;
		let tx = &tx.tx;
		transaction.put_owned(
			node_db::columns::TX,
			DBKey::from_slice(&tx_hash.0),
			codec::encode(&tx)?,
		);
	}
	for tx in &commit_block_params.payload_txs {
		let tx_hash = &tx.tx_hash;
		let tx = &tx.tx;
		transaction.put_owned(
			node_db::columns::TX,
			DBKey::from_slice(&tx_hash.0),
			codec::encode(&tx)?,
		);
	}

	// 5. meta receipts
	for receipt in &commit_block_params.meta_receipts {
		let tx_hash = &receipt.tx_hash;
		let receipt = &receipt.receipt;
		transaction.put_owned(
			node_db::columns::RECEIPT,
			DBKey::from_slice(&tx_hash.0),
			codec::encode(&receipt)?,
		);
	}

	// 6. block hash
	transaction.put_owned(
		node_db::columns::BLOCK_HASH,
		DBKey::from_slice(&codec::encode(&commit_block_params.header.number)?),
		codec::encode(&commit_block_params.block_hash)?,
	);

	// 7. confirmed number
	transaction.put_owned(
		node_db::columns::GLOBAL,
		DBKey::from_slice(node_db::global_key::CONFIRMED_NUMBER),
		codec::encode(&commit_block_params.header.number)?,
	);
	Ok(())
}

fn commit_executed(
	transaction: &mut DBTransaction,
	commit_execute_params: ChainCommitExecuteParams,
) -> CommonResult<()> {
	// 1. payload state
	transaction.extend(commit_execute_params.payload_transaction);

	// 2. executed
	transaction.put_owned(
		node_db::columns::EXECUTED,
		DBKey::from_slice(&commit_execute_params.block_hash.0),
		codec::encode(&commit_execute_params.executed)?,
	);

	// 3. payload receipts
	for receipt in &commit_execute_params.payload_receipts {
		let tx_hash = &receipt.tx_hash;
		let receipt = &receipt.receipt;
		transaction.put_owned(
			node_db::columns::RECEIPT,
			DBKey::from_slice(&tx_hash.0),
			codec::encode(&receipt)?,
		);
	}

	// 4. executed number
	transaction.put_owned(
		node_db::columns::GLOBAL,
		DBKey::from_slice(node_db::global_key::EXECUTED_NUMBER),
		codec::encode(&commit_execute_params.number)?,
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
