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

use smallvec::alloc::sync::Arc;
use smallvec::SmallVec;

use crate::codec::{Decode, Encode};

#[derive(Clone, Encode, Decode, PartialEq)]
pub struct Address(pub Vec<u8>);

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct PublicKey(pub Vec<u8>);

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct SecretKey(pub Vec<u8>);

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
/// signature for (nonce, until, call)
pub struct Signature(pub Vec<u8>);

pub type Nonce = u32;

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct Witness {
	pub public_key: PublicKey,
	pub signature: Signature,
	pub nonce: Nonce,
	pub until: BlockNumber,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct Params(pub Vec<u8>);

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct Call {
	pub module: String,
	pub method: String,
	pub params: Params,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct Transaction {
	pub witness: Option<Witness>,
	pub call: Call,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, Hash)]
pub struct Hash(pub Vec<u8>);

pub type BlockNumber = u64;

pub type Balance = u64;

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct Header {
	pub number: BlockNumber,
	pub timestamp: u64,
	pub parent_hash: Hash,
	pub meta_txs_root: Hash,
	pub meta_state_root: Hash,
	pub meta_receipts_root: Hash,
	pub payload_txs_root: Hash,
	pub payload_execution_gap: i8,
	pub payload_execution_state_root: Hash,
	pub payload_execution_receipts_root: Hash,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct Body {
	pub meta_txs: Vec<Hash>,
	pub payload_txs: Vec<Hash>,
}

#[derive(Debug, PartialEq)]
pub struct Block {
	pub header: Header,
	pub body: Body,
}

#[derive(Debug, PartialEq)]
pub struct FullTransaction {
	pub tx: Transaction,
	pub tx_hash: Hash,
}

#[derive(Debug, PartialEq)]
pub struct FullReceipt {
	pub receipt: Receipt,
	pub tx_hash: Hash,
}

pub type CallResult<T> = Result<T, String>;

pub type TransactionResult = Result<Vec<u8>, String>;

#[derive(Clone, Encode, Decode, Debug, PartialEq)]
pub struct Event(pub Vec<u8>);

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Receipt {
	pub block_number: BlockNumber,
	pub events: Vec<Event>,
	pub result: TransactionResult,
}

#[derive(Debug, PartialEq)]
pub struct CommitBlockParams<T> {
	pub block_hash: Hash,
	pub header: Header,
	pub body: Body,
	pub meta_txs: Vec<Arc<FullTransaction>>,
	pub meta_receipts: Vec<Arc<FullReceipt>>,
	pub payload_txs: Vec<Arc<FullTransaction>>,
	pub meta_transaction: T,
}

#[derive(Debug, PartialEq)]
pub struct BuildBlockParams {
	pub number: BlockNumber,
	pub timestamp: u64,
	pub meta_txs: Vec<Arc<FullTransaction>>,
	pub payload_txs: Vec<Arc<FullTransaction>>,
}

#[derive(Debug, PartialEq)]
pub struct CommitExecutionParams<T> {
	pub block_hash: Hash,
	pub number: BlockNumber,
	pub execution: Execution,
	pub payload_receipts: Vec<Arc<FullReceipt>>,
	pub payload_transaction: T,
}

#[derive(Debug, PartialEq)]
pub struct BuildExecutionParams {
	pub number: BlockNumber,
	pub timestamp: u64,
	pub block_hash: Hash,
	pub meta_state_root: Hash,
	pub payload_state_root: Hash,
	pub payload_txs: Vec<Arc<FullTransaction>>,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct Execution {
	pub payload_execution_state_root: Hash,
	pub payload_execution_receipts_root: Hash,
}

pub type DBKey = SmallVec<[u8; 32]>;
pub type DBValue = Vec<u8>;

/// exclude signature to avoid malleability
#[derive(Encode)]
pub struct TransactionForHash<'a> {
	pub witness: Option<(&'a PublicKey, &'a Nonce, &'a BlockNumber)>,
	pub call: &'a Call,
}
