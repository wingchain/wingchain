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

use std::collections::HashMap;
use std::sync::Arc;

use parity_codec::Encode;

use hash_enum::HashEnum;
use node_db::{DBKey, DBValue};
use node_executor_primitives::{Context as ContextT, Value, Error};
use node_statedb::{StateDB, StateDBGetter, StateDBStmt};
use primitives::{BlockNumber, Call, Params, traits::Module as ModuleT, Transaction};
use std::io::ErrorKind;
use std::rc::Rc;
use std::cell::RefCell;

pub mod errors;

#[derive(Clone)]
pub struct Context {
	inner: Rc<ContextInner>,
}

struct ContextInner {
	number: BlockNumber,
	timestamp: u32,
	meta_state: ContextState,
	payload_state: ContextState,
}

impl Context {
	pub fn new(number: BlockNumber,
			   timestamp: u32,
			   meta_statedb: Arc<StateDB>,
			   meta_state_root: Vec<u8>,
			   payload_statedb: Arc<StateDB>,
			   payload_state_root: Vec<u8>,
	) -> errors::Result<Self> {
		let meta_state = ContextState::new(meta_statedb, meta_state_root)?;
		let payload_state = ContextState::new(payload_statedb, payload_state_root)?;

		let inner = Rc::new(ContextInner{
			number,
			timestamp,
			meta_state,
			payload_state,
		});

		Ok(Self {
			inner
		})
	}
}

struct ContextState {
	/// statedb_stmt is referred by statedb_getter, should be kept
	statedb_stmt: StateDBStmt,
	/// unsafe, should never out live lib
	statedb_getter: StateDBGetter<'static>,
	buffer: RefCell<HashMap<DBKey, Option<DBValue>>>,
}

impl ContextState {
	fn new(statedb: Arc<StateDB>,
		   state_root: Vec<u8>) -> errors::Result<Self> {
		let statedb_stmt = statedb.prepare_stmt(&state_root)?;
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
	fn meta_get(&self, key: &[u8]) -> Result<Option<Value>, Error> {
		let buffer = self.inner.meta_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => {
				self.inner.meta_state.statedb_getter.get(key).map_err(|_|ErrorKind::InvalidData.into())
			}
		}

	}
	fn meta_set(&self, key: &[u8], value: Option<Value>) -> Result<(), Error> {
		let mut buffer = self.inner.meta_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
	fn payload_get(&self, key: &[u8]) -> Result<Option<Value>, Error> {
		let buffer = self.inner.payload_state.buffer.borrow();
		match buffer.get(&DBKey::from_slice(key)) {
			Some(value) => Ok(value.clone()),
			None => {
				self.inner.payload_state.statedb_getter.get(key).map_err(|_|ErrorKind::InvalidData.into())
			}
		}
	}
	fn payload_set(&self, key: &[u8], value: Option<Value>) -> Result<(), Error> {
		let mut buffer = self.inner.payload_state.buffer.borrow_mut();
		buffer.insert(DBKey::from_slice(key), value);
		Ok(())
	}
}

pub struct Executor;

impl Executor {
	pub fn build_tx<M: HashEnum, P: Encode>(
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
			ModuleEnum::System => <module::system::Module::<Context> as ModuleT>::validate_call(&call),
		};

		match valid {
			true => Some(Transaction {
				witness: None,
				call,
			}),
			false => None,
		}
	}

	pub fn execute_txs(context: &mut Context, txs: Vec<Transaction>) -> errors::Result<Vec<u8>> {
		unimplemented!()
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
