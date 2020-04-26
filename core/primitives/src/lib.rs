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

use std::fmt;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::errors::{CommonError, CommonErrorKind, CommonResult};

pub mod codec;
pub mod errors;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Address(pub Vec<u8>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
/// signature for (nonce, expire, call)
pub struct Signature(pub Vec<u8>);

pub type Nonce = u32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Witness {
	pub address: Address,
	pub signature: Signature,
	pub nonce: Nonce,
	pub expire: BlockNumber,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Params(pub Vec<u8>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Call {
	pub module: String,
	pub method: String,
	pub params: Params,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
	pub witness: Option<Witness>,
	pub call: Call,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Hash)]
pub struct Hash(pub Vec<u8>);

pub type BlockNumber = u32;

pub type Balance = u64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Header {
	pub number: BlockNumber,
	pub timestamp: u32,
	pub parent_hash: Hash,
	pub meta_txs_root: Hash,
	pub meta_state_root: Hash,
	pub payload_txs_root: Hash,
	pub payload_executed_gap: i8,
	pub payload_executed_state_root: Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Body {
	pub meta_txs: Vec<Hash>,
	pub payload_txs: Vec<Hash>,
}

#[derive(Debug, PartialEq)]
pub struct Block {
	pub header: Header,
	pub body: Body,
}

pub struct FullBlock {
	pub number: BlockNumber,
	pub block_hash: Hash,
	pub header: Header,
	pub body: Body,
	pub txs: Vec<(Hash, Transaction)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Executed {
	pub payload_executed_state_root: Hash,
}

pub type DBKey = SmallVec<[u8; 32]>;
pub type DBValue = Vec<u8>;

impl fmt::Debug for Hash {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", hex::encode(&self.0))
	}
}

impl Hash {
	pub fn from_hex(hex: &str) -> CommonResult<Self> {
		let hex =
			hex::decode(hex).map_err(|e| CommonError::new(CommonErrorKind::Codec, Box::new(e)))?;
		Ok(Hash(hex))
	}
}
