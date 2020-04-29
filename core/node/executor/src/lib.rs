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

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use serde::Serialize;

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{Dsa, DsaImpl, Verifier};
use node_db::DBTransaction;
use node_executor_macro::dispatcher;
pub use node_executor_primitives::ContextEnv;
use node_executor_primitives::{Context as ContextT, Module as ModuleT};
use node_statedb::{StateDB, StateDBGetter, StateDBStmt, TrieRoot};
use primitives::{codec, errors::CommonResult, TransactionForHash};
use primitives::{Address, Call, DBKey, DBValue, Hash, Params, Transaction};

pub mod errors;

const META_TXS_SIZE: usize = 64;
const PAYLOAD_TXS_SIZE: usize = 512;

pub struct ContextEssence {
	env: Rc<ContextEnv>,
	trie_root: Arc<TrieRoot>,
	meta_statedb: Arc<StateDB>,
	meta_state_root: Rc<Hash>,
	meta_stmt: StateDBStmt,
	payload_statedb: Arc<StateDB>,
	payload_state_root: Rc<Hash>,
	payload_stmt: StateDBStmt,
}

impl ContextEssence {
	pub fn new(
		env: ContextEnv,
		trie_root: Arc<TrieRoot>,
		meta_statedb: Arc<StateDB>,
		meta_state_root: Hash,
		payload_statedb: Arc<StateDB>,
		payload_state_root: Hash,
	) -> CommonResult<Self> {
		let meta_stmt = meta_statedb.prepare_stmt(&meta_state_root.0)?;
		let payload_stmt = payload_statedb.prepare_stmt(&payload_state_root.0)?;

		Ok(Self {
			env: Rc::new(env),
			trie_root,
			meta_statedb,
			meta_state_root: Rc::new(meta_state_root),
			meta_stmt,
			payload_statedb,
			payload_state_root: Rc::new(payload_state_root),
			payload_stmt,
		})
	}
}

#[derive(Clone)]
pub struct Context<'a> {
	inner: Rc<ContextInner<'a>>,
}

struct ContextInner<'a> {
	env: Rc<ContextEnv>,
	trie_root: Arc<TrieRoot>,
	meta_statedb: Arc<StateDB>,
	meta_state_root: Rc<Hash>,
	meta_state: ContextState<'a>,
	meta_txs: RefCell<Vec<Arc<Transaction>>>,
	payload_statedb: Arc<StateDB>,
	payload_state_root: Rc<Hash>,
	payload_state: ContextState<'a>,
	payload_txs: RefCell<Vec<Arc<Transaction>>>,
	// to mark the context has already started to executed payload txs
	payload_phase: Cell<bool>,
}

struct ContextState<'a> {
	statedb_getter: StateDBGetter<'a>,
	buffer: RefCell<HashMap<DBKey, Option<DBValue>>>,
}

impl<'a> ContextState<'a> {
	fn new(statedb_stmt: &'a StateDBStmt) -> CommonResult<Self> {
		let statedb_getter = StateDB::prepare_get(statedb_stmt)?;
		let buffer = Default::default();

		Ok(ContextState {
			statedb_getter,
			buffer,
		})
	}
}

impl<'a> ContextT for Context<'a> {
	fn env(&self) -> Rc<ContextEnv> {
		self.inner.env.clone()
	}
	fn meta_get(&self, key: &[u8]) -> CommonResult<Option<DBValue>> {
		let buffer = self.inner.meta_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => self.inner.meta_state.statedb_getter.get(key),
		}
	}
	fn meta_set(&self, key: &[u8], value: Option<DBValue>) -> CommonResult<()> {
		let mut buffer = self.inner.meta_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_get(&self, key: &[u8]) -> CommonResult<Option<DBValue>> {
		let buffer = self.inner.payload_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => self.inner.payload_state.statedb_getter.get(key),
		}
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> CommonResult<()> {
		let mut buffer = self.inner.payload_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
}

impl<'a> Context<'a> {
	pub fn new(context_essence: &'a ContextEssence) -> CommonResult<Self> {
		let meta_state = ContextState::new(&context_essence.meta_stmt)?;
		let payload_state = ContextState::new(&context_essence.payload_stmt)?;

		let inner = Rc::new(ContextInner {
			env: context_essence.env.clone(),
			trie_root: context_essence.trie_root.clone(),
			meta_statedb: context_essence.meta_statedb.clone(),
			meta_state_root: context_essence.meta_state_root.clone(),
			meta_state,
			meta_txs: RefCell::new(Vec::with_capacity(META_TXS_SIZE)),
			payload_statedb: context_essence.payload_statedb.clone(),
			payload_state_root: context_essence.payload_state_root.clone(),
			payload_state,
			payload_txs: RefCell::new(Vec::with_capacity(PAYLOAD_TXS_SIZE)),
			payload_phase: Cell::new(false),
		});

		Ok(Self { inner })
	}

	pub fn get_meta_update(&self) -> CommonResult<(Hash, DBTransaction)> {
		let buffer = self.inner.meta_state.buffer.borrow();
		let (root, transaction) = self
			.inner
			.meta_statedb
			.prepare_update(&self.inner.meta_state_root.0, buffer.iter())?;
		Ok((Hash(root), transaction))
	}

	pub fn get_meta_txs(&self) -> CommonResult<(Hash, Vec<Arc<Transaction>>)> {
		let txs = self.inner.meta_txs.borrow().clone();

		let input = txs
			.iter()
			.map(|x| codec::encode(&TransactionForHash::new(&**x)))
			.collect::<Result<Vec<Vec<u8>>, _>>()?;
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);

		Ok((Hash(txs_root), txs))
	}

	pub fn get_payload_update(&self) -> CommonResult<(Hash, DBTransaction)> {
		let buffer = self.inner.payload_state.buffer.borrow();
		let (root, transaction) = self
			.inner
			.payload_statedb
			.prepare_update(&self.inner.payload_state_root.0, buffer.iter())?;
		Ok((Hash(root), transaction))
	}

	pub fn get_payload_txs(&self) -> CommonResult<(Hash, Vec<Arc<Transaction>>)> {
		let txs = self.inner.payload_txs.borrow().clone();

		let input = txs
			.iter()
			.map(|x| codec::encode(&TransactionForHash::new(&**x)))
			.collect::<Result<Vec<Vec<u8>>, _>>()?;
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);

		Ok((Hash(txs_root), txs))
	}
}

/*
#[derive(Clone)]
pub struct Context {
	inner: Rc<ContextInner>,
}

struct ContextInner {
	env: Rc<ContextEnv>,
	trie_root: Arc<TrieRoot>,
	meta_statedb: Arc<StateDB>,
	meta_state_root: Hash,
	meta_state: ContextState,
	meta_txs: RefCell<Vec<Arc<Transaction>>>,
	payload_statedb: Arc<StateDB>,
	payload_state_root: Hash,
	payload_state: ContextState,
	payload_txs: RefCell<Vec<Arc<Transaction>>>,
	// to mark the context has already started to executed payload txs
	payload_phase: Cell<bool>,
}

impl Context {
	pub fn new(
		env: Rc<ContextEnv>,
		trie_root: Arc<TrieRoot>,
		meta_statedb: Arc<StateDB>,
		meta_state_root: Hash,
		payload_statedb: Arc<StateDB>,
		payload_state_root: Hash,
	) -> CommonResult<Self> {
		let meta_state = ContextState::new(meta_statedb.clone(), &meta_state_root.0)?;
		let payload_state = ContextState::new(payload_statedb.clone(), &payload_state_root.0)?;

		let inner = Rc::new(ContextInner {
			env,
			trie_root,
			meta_statedb,
			meta_state_root,
			meta_state,
			meta_txs: RefCell::new(Vec::with_capacity(META_TXS_SIZE)),
			payload_statedb,
			payload_state_root,
			payload_state,
			payload_txs: RefCell::new(Vec::with_capacity(PAYLOAD_TXS_SIZE)),
			payload_phase: Cell::new(false),
		});

		Ok(Self { inner })
	}

	pub fn get_meta_update(&self) -> CommonResult<(Hash, DBTransaction)> {
		let buffer = self.inner.meta_state.buffer.borrow();
		let (root, transaction) = self
			.inner
			.meta_statedb
			.prepare_update(&self.inner.meta_state_root.0, buffer.iter())?;
		Ok((Hash(root), transaction))
	}

	pub fn get_meta_txs(&self) -> CommonResult<(Hash, Vec<Arc<Transaction>>)> {
		let txs = self.inner.meta_txs.borrow().clone();

		let input = txs
			.iter()
			.map(|x| codec::encode(&**x))
			.collect::<Result<Vec<Vec<u8>>, _>>()?;
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);

		Ok((Hash(txs_root), txs))
	}

	pub fn get_payload_update(&self) -> CommonResult<(Hash, DBTransaction)> {
		let buffer = self.inner.payload_state.buffer.borrow();
		let (root, transaction) = self
			.inner
			.payload_statedb
			.prepare_update(&self.inner.payload_state_root.0, buffer.iter())?;
		Ok((Hash(root), transaction))
	}

	pub fn get_payload_txs(&self) -> CommonResult<(Hash, Vec<Arc<Transaction>>)> {
		let txs = self.inner.payload_txs.borrow().clone();

		let input = txs
			.iter()
			.map(|x| codec::encode(&**x))
			.collect::<Result<Vec<Vec<u8>>, _>>()?;
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);

		Ok((Hash(txs_root), txs))
	}
}

struct ContextState {
	#[allow(dead_code)]
	/// statedb_stmt is referred by statedb_getter, should be kept
	statedb_stmt: StateDBStmt,
	/// unsafe, should never out live statedb_stmt
	statedb_getter: StateDBGetter<'static>,
	buffer: RefCell<HashMap<DBKey, Option<DBValue>>>,
}

impl ContextState {
	fn new(statedb: Arc<StateDB>, state_root: &[u8]) -> CommonResult<Self> {
		let statedb_stmt = statedb.prepare_stmt(state_root)?;
		let statedb_getter = StateDB::prepare_get(&statedb_stmt)?;
		let buffer = Default::default();

		let statedb_getter = unsafe {
			std::mem::transmute::<StateDBGetter<'_>, StateDBGetter<'static>>(statedb_getter)
		};

		Ok(ContextState {
			statedb_stmt,
			statedb_getter,
			buffer,
		})
	}
}

impl ContextT for Context {
	fn env(&self) -> Rc<ContextEnv> {
		self.inner.env.clone()
	}
	fn meta_get(&self, key: &[u8]) -> CommonResult<Option<DBValue>> {
		let buffer = self.inner.meta_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => self.inner.meta_state.statedb_getter.get(key),
		}
	}
	fn meta_set(&self, key: &[u8], value: Option<DBValue>) -> CommonResult<()> {
		let mut buffer = self.inner.meta_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_get(&self, key: &[u8]) -> CommonResult<Option<DBValue>> {
		let buffer = self.inner.payload_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => self.inner.payload_state.statedb_getter.get(key),
		}
	}
	fn payload_set(&self, key: &[u8], value: Option<DBValue>) -> CommonResult<()> {
		let mut buffer = self.inner.payload_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
}
*/

pub struct Executor {
	dsa: Arc<DsaImpl>,
	address: Arc<AddressImpl>,
}

impl Executor {
	pub fn new(dsa: Arc<DsaImpl>, address: Arc<AddressImpl>) -> Self {
		Self { dsa, address }
	}

	pub fn build_tx<P: Serialize>(
		&self,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<Transaction> {
		let params = Params(codec::encode(&params)?);

		let call = Call {
			module: module.clone(),
			method: method,
			params,
		};

		let valid = Dispatcher::is_valid_call::<Context>(&module, &call)?;

		if !valid {
			return Err(errors::ErrorKind::InvalidTxCall.into());
		}

		Ok(Transaction {
			witness: None,
			call,
		})
	}

	pub fn validate_tx(&self, tx: &Transaction) -> CommonResult<()> {
		let witness = tx
			.witness
			.as_ref()
			.ok_or(errors::ErrorKind::InvalidTxWitness(
				"missing witness".to_string(),
			))?;
		let call = &tx.call;

		let signature = &witness.signature;
		let message = codec::encode(&(&witness.nonce, &witness.until, call))?;

		let verifier = self.dsa.verifier_from_public_key(&witness.public_key.0)?;

		verifier
			.verify(&message, &signature.0)
			.map_err(|_| errors::ErrorKind::InvalidTxWitness("invalid signature".to_string()))?;

		let module = &call.module;

		let valid = Dispatcher::is_valid_call::<Context>(module, &call)?;

		let write = Dispatcher::is_write_call::<Context>(module, &call)?;

		if !(valid && write == Some(true)) {
			return Err(errors::ErrorKind::InvalidTxCall.into());
		}

		Ok(())
	}

	pub fn validate_address(&self, address: &Address) -> CommonResult<()> {
		let expected_address_len: usize = self.address.length().into();
		if address.0.len() != expected_address_len {
			return Err(errors::ErrorKind::InvalidAddress.into());
		}
		Ok(())
	}

	pub fn is_meta_tx(&self, tx: &Transaction) -> CommonResult<bool> {
		let module = &tx.call.module;
		Dispatcher::is_meta::<Context>(module)
	}

	pub fn execute_txs(&self, context: &Context, txs: Vec<Arc<Transaction>>) -> CommonResult<()> {
		let mut txs_is_meta: Option<bool> = None;
		for tx in &txs {
			let call = &tx.call;
			let module = &call.module;

			let is_meta = Dispatcher::is_meta::<Context>(module)?;
			match txs_is_meta {
				None => {
					txs_is_meta = Some(is_meta);
				}
				Some(txs_is_meta) => {
					if txs_is_meta != is_meta {
						return Err(errors::ErrorKind::InvalidTxs(
							"mixed meta and payload in one txs batch".to_string(),
						)
						.into());
					}
				}
			}

			let sender = tx.witness.as_ref().map(|witness| {
				let public_key = &witness.public_key;
				let mut address = vec![0u8; self.address.length().into()];
				self.address.address(&mut address, &public_key.0);
				Address(address)
			});

			let _result =
				Dispatcher::execute_call::<Context>(module, context, sender.as_ref(), &call)?;
		}

		if context.inner.payload_phase.get() && txs_is_meta == Some(true) {
			return Err(errors::ErrorKind::InvalidTxs(
				"meta after payload not allowed".to_string(),
			)
			.into());
		}

		let mut txs = txs;
		match txs_is_meta {
			Some(true) => context.inner.meta_txs.borrow_mut().append(&mut txs),
			Some(false) => context.inner.payload_txs.borrow_mut().append(&mut txs),
			_ => (),
		}

		Ok(())
	}
}

#[allow(non_camel_case_types)]
#[dispatcher]
enum Dispatcher {
	system,
	balance,
}

/// re-import modules
pub mod module {
	pub use module_balance as balance;
	pub use module_system as system;
}
