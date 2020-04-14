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

use parity_codec::Encode;

use hash_enum::HashEnum;
use node_db::{DBKey, DBTransaction, DBValue};
use node_executor_primitives::{Context as ContextT, Module as ModuleT};
use node_statedb::{StateDB, StateDBGetter, StateDBStmt, TrieRoot};
use primitives::{BlockNumber, Call, FromDispatchId, Params, Transaction};

pub mod errors;

const META_TXS_SIZE: usize = 64;
const PAYLOAD_TXS_SIZE: usize = 512;

#[derive(Clone)]
pub struct Context {
	inner: Rc<ContextInner>,
}

struct ContextInner {
	number: BlockNumber,
	timestamp: u32,
	trie_root: Arc<TrieRoot>,
	meta_statedb: Arc<StateDB>,
	meta_state_root: Vec<u8>,
	meta_state: ContextState,
	meta_txs: RefCell<Vec<Arc<Transaction>>>,
	payload_statedb: Arc<StateDB>,
	payload_state_root: Vec<u8>,
	payload_state: ContextState,
	payload_txs: RefCell<Vec<Arc<Transaction>>>,
	// to mark the context has already started to executed payload txs
	payload_phase: Cell<bool>,
}

impl Context {
	pub fn new(
		number: BlockNumber,
		timestamp: u32,
		trie_root: Arc<TrieRoot>,
		meta_statedb: Arc<StateDB>,
		meta_state_root: Vec<u8>,
		payload_statedb: Arc<StateDB>,
		payload_state_root: Vec<u8>,
	) -> errors::Result<Self> {
		let meta_state = ContextState::new(meta_statedb.clone(), &meta_state_root)?;
		let payload_state = ContextState::new(payload_statedb.clone(), &payload_state_root)?;

		let inner = Rc::new(ContextInner {
			number,
			timestamp,
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

	pub fn get_meta_update(&self) -> errors::Result<(Vec<u8>, DBTransaction)> {
		let buffer = self.inner.meta_state.buffer.borrow();
		let result = self
			.inner
			.meta_statedb
			.prepare_update(&self.inner.meta_state_root, buffer.iter())?;
		Ok(result)
	}

	pub fn get_meta_txs(&self) -> errors::Result<(Vec<u8>, Vec<Arc<Transaction>>)> {
		let txs = self.inner.meta_txs.borrow().clone();

		let input = txs.iter().map(|x| Encode::encode(&x));
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);

		Ok((txs_root, txs))
	}

	pub fn get_payload_update(&self) -> errors::Result<(Vec<u8>, DBTransaction)> {
		let buffer = self.inner.payload_state.buffer.borrow();
		let result = self
			.inner
			.payload_statedb
			.prepare_update(&self.inner.payload_state_root, buffer.iter())?;
		Ok(result)
	}

	pub fn get_payload_txs(&self) -> errors::Result<(Vec<u8>, Vec<Arc<Transaction>>)> {
		let txs = self.inner.payload_txs.borrow().clone();

		let input = txs.iter().map(|x| Encode::encode(&x));
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);

		Ok((txs_root, txs))
	}
}

struct ContextState {
	#[allow(dead_code)]
	/// statedb_stmt is referred by statedb_getter, should be kept
	statedb_stmt: StateDBStmt,
	/// unsafe, should never out live lib
	statedb_getter: StateDBGetter<'static>,
	buffer: RefCell<HashMap<DBKey, Option<DBValue>>>,
}

impl ContextState {
	fn new(statedb: Arc<StateDB>, state_root: &[u8]) -> errors::Result<Self> {
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
	fn meta_get(&self, key: &[u8]) -> node_executor_primitives::errors::Result<Option<DBValue>> {
		let buffer = self.inner.meta_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => self
				.inner
				.meta_state
				.statedb_getter
				.get(key)
				.map_err(|_| node_executor_primitives::errors::ErrorKind::TrieError.into()),
		}
	}
	fn meta_set(
		&self,
		key: &[u8],
		value: Option<DBValue>,
	) -> node_executor_primitives::errors::Result<()> {
		let mut buffer = self.inner.meta_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_get(&self, key: &[u8]) -> node_executor_primitives::errors::Result<Option<DBValue>> {
		let buffer = self.inner.payload_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => self
				.inner
				.payload_state
				.statedb_getter
				.get(key)
				.map_err(|_| node_executor_primitives::errors::ErrorKind::TrieError.into()),
		}
	}
	fn payload_set(
		&self,
		key: &[u8],
		value: Option<DBValue>,
	) -> node_executor_primitives::errors::Result<()> {
		let mut buffer = self.inner.payload_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
}

pub struct Executor;

impl Executor {
	pub fn new() -> Self {
		Self
	}

	pub fn build_tx<M: HashEnum, P: Encode>(
		&self,
		module: ModuleEnum,
		method: M,
		params: P,
	) -> Option<Transaction> {
		let call = Call {
			module_id: module.clone().into(),
			method_id: method.into(),
			params: Params(params.encode()),
		};

		let valid = match module {
			ModuleEnum::System => module::system::Module::<Context>::validate_call(&call),
		};

		match valid {
			true => Some(Transaction {
				witness: None,
				call,
			}),
			false => None,
		}
	}

	pub fn execute_txs(&self, context: &Context, txs: Vec<Arc<Transaction>>) -> errors::Result<()> {
		let mut txs_is_meta: Option<bool> = None;
		for tx in &txs {
			let module_id = &tx.call.module_id;
			let module_enum = ModuleEnum::from_dispatch_id(module_id).ok_or(
				errors::ErrorKind::InvalidDispatchId(tx.call.module_id.clone()),
			)?;
			let call = &tx.call;
			let (_result, is_meta) = match module_enum {
				ModuleEnum::System => {
					let is_meta =
						<module::system::Module<Context> as ModuleT<Context>>::META_MODULE;
					let module = module::system::Module::new(context.clone());
					let result = module.execute_call(call);
					(result, is_meta)
				}
			};
			match txs_is_meta {
				None => {
					txs_is_meta = Some(is_meta);
				}
				Some(txs_is_meta) => {
					if txs_is_meta != is_meta {
						return Err(errors::ErrorKind::MixedTxs.into());
					}
				}
			}
		}

		if context.inner.payload_phase.get() && txs_is_meta == Some(true) {
			return Err(errors::ErrorKind::IllegalTxsPhase.into());
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

#[derive(HashEnum, Clone)]
pub enum ModuleEnum {
	System,
}

/// re-import modules
pub mod module {
	pub use module_system as system;
}
