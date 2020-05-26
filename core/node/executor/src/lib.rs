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

use crypto::address::{Address as AddressT, AddressImpl};
use crypto::dsa::{Dsa, DsaImpl, KeyPair, Verifier};
use crypto::hash::Hash as HashT;
use crypto::hash::HashImpl;
use node_db::DBTransaction;
use node_executor_macro::dispatcher;
pub use node_executor_primitives::CallResult;
pub use node_executor_primitives::ContextEnv;
use node_executor_primitives::{
	errors, Context as ContextT, Module as ModuleT, Validator as ValidatorT,
};
use node_statedb::{StateDB, StateDBGetter, StateDBStmt, TrieRoot};
use primitives::codec::Encode;
use primitives::{
	codec, errors::CommonResult, BlockNumber, FullTransaction, Nonce, PublicKey, SecretKey,
	Signature, TransactionForHash, Witness,
};
use primitives::{Address, Call, DBKey, DBValue, Hash, Params, Transaction};

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
	meta_txs: RefCell<Vec<Arc<FullTransaction>>>,
	payload_statedb: Arc<StateDB>,
	payload_state_root: Rc<Hash>,
	payload_state: ContextState<'a>,
	payload_txs: RefCell<Vec<Arc<FullTransaction>>>,
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

	pub fn get_meta_txs(&self) -> CommonResult<(Hash, Vec<Arc<FullTransaction>>)> {
		let txs = self.inner.meta_txs.borrow().clone();

		let input = txs
			.iter()
			.map(|x| codec::encode(&TransactionForHash::new(&x.tx)))
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

	pub fn get_payload_txs(&self) -> CommonResult<(Hash, Vec<Arc<FullTransaction>>)> {
		let txs = self.inner.payload_txs.borrow().clone();

		let input = txs
			.iter()
			.map(|x| codec::encode(&TransactionForHash::new(&x.tx)))
			.collect::<Result<Vec<Vec<u8>>, _>>()?;
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);

		Ok((Hash(txs_root), txs))
	}

	pub fn get_txs_root(&self, txs: &Vec<Arc<FullTransaction>>) -> CommonResult<Hash> {
		let input = txs
			.iter()
			.map(|x| codec::encode(&TransactionForHash::new(&x.tx)))
			.collect::<Result<Vec<Vec<u8>>, _>>()?;
		let txs_root = self.inner.trie_root.calc_ordered_trie_root(input);
		Ok(Hash(txs_root))
	}
}

struct Validator {
	address: Arc<AddressImpl>,
}

impl ValidatorT for Validator {
	fn validate_address(&self, address: &Address) -> CommonResult<()> {
		let address_len: usize = self.address.length().into();
		if address.0.len() != address_len {
			return Err(errors::ErrorKind::InvalidAddress(format!("{}", address)).into());
		}
		Ok(())
	}
}

pub struct Executor {
	hash: Arc<HashImpl>,
	dsa: Arc<DsaImpl>,
	address: Arc<AddressImpl>,
	validator: Validator,
}

impl Executor {
	pub fn new(hash: Arc<HashImpl>, dsa: Arc<DsaImpl>, address: Arc<AddressImpl>) -> Self {
		let validator = Validator {
			address: address.clone(),
		};
		Self {
			hash,
			dsa,
			address,
			validator,
		}
	}

	pub fn build_tx<P: Encode>(
		&self,
		witness: Option<(SecretKey, Nonce, BlockNumber)>,
		module: String,
		method: String,
		params: P,
	) -> CommonResult<Transaction> {
		let params = Params(codec::encode(&params)?);

		let call = Call {
			module: module.clone(),
			method,
			params,
		};

		Dispatcher::validate_call::<Context, _>(&module, &self.validator, &call)?;

		let witness = match witness {
			Some((secret_key, nonce, until)) => {
				let message = codec::encode(&(&nonce, &until, &call))?;
				let key_pair = self.dsa.key_pair_from_secret_key(&secret_key.0)?;
				let (_, pub_len, sig_len) = self.dsa.length().into();
				let public_key = {
					let mut out = vec![0u8; pub_len];
					key_pair.public_key(&mut out);
					PublicKey(out)
				};
				let signature = {
					let mut out = vec![0u8; sig_len];
					key_pair.sign(&message, &mut out);
					Signature(out)
				};
				Some(Witness {
					public_key,
					signature,
					nonce,
					until,
				})
			}
			None => None,
		};

		Ok(Transaction { witness, call })
	}

	pub fn validate_tx(&self, tx: &Transaction, witness_required: bool) -> CommonResult<()> {
		let call = &tx.call;

		match &tx.witness {
			Some(witness) => {
				let signature = &witness.signature;
				let message = codec::encode(&(&witness.nonce, &witness.until, call))?;
				let verifier = self.dsa.verifier_from_public_key(&witness.public_key.0)?;
				verifier.verify(&message, &signature.0).map_err(|_| {
					errors::ErrorKind::InvalidTxWitness("invalid signature".to_string())
				})?;
			}
			None => {
				if witness_required {
					return Err(
						errors::ErrorKind::InvalidTxWitness("missing witness".to_string()).into(),
					);
				}
			}
		};

		let module = &call.module;
		Dispatcher::validate_call::<Context, _>(module, &self.validator, &call)?;

		let write = Dispatcher::is_write_call::<Context>(module, &call)?;
		if write != Some(true) {
			return Err(
				errors::ErrorKind::InvalidTxMethod(format!("{}: not write", call.method)).into(),
			);
		}

		Ok(())
	}

	pub fn is_meta_tx(&self, tx: &Transaction) -> CommonResult<bool> {
		let module = &tx.call.module;
		Dispatcher::is_meta::<Context>(module)
	}

	pub fn execute_call(
		&self,
		context: &Context,
		sender: Option<&Address>,
		call: &Call,
	) -> CommonResult<CommonResult<CallResult>> {
		let module = &call.module;
		Dispatcher::execute_call::<Context>(module, context, sender, call)
	}

	pub fn execute_txs(
		&self,
		context: &Context,
		txs: Vec<Arc<FullTransaction>>,
	) -> CommonResult<()> {
		let mut txs_is_meta: Option<bool> = None;
		for tx in &txs {
			let tx = &tx.tx;
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

		if txs_is_meta == Some(false) {
			context.inner.payload_phase.set(true);
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

	pub fn hash_transaction(&self, tx: &Transaction) -> CommonResult<Hash> {
		let transaction_for_hash = TransactionForHash::new(tx);
		self.hash(&transaction_for_hash)
	}

	pub fn hash<D: Encode>(&self, data: &D) -> CommonResult<Hash> {
		let encoded = codec::encode(data)?;
		Ok(self.hash_slice(&encoded))
	}

	pub fn default_hash(&self) -> Hash {
		Hash(vec![0u8; self.hash.length().into()])
	}

	fn hash_slice(&self, data: &[u8]) -> Hash {
		let mut out = vec![0u8; self.hash.length().into()];
		self.hash.hash(&mut out, data);
		Hash(out)
	}
}

#[allow(non_camel_case_types)]
#[dispatcher]
enum Dispatcher {
	system,
	balance,
	solo,
}

/// re-import modules
pub mod module {
	pub use module_balance as balance;
	pub use module_solo as solo;
	pub use module_system as system;
}
