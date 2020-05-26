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

use crate::codec::{self, Encode};
use crate::errors::{CommonError, CommonErrorKind, CommonResult};
use crate::types::{Address, Event, Hash, Transaction, TransactionForHash};

impl fmt::Debug for Hash {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", hex::encode(&self.0))
	}
}

impl fmt::Display for Hash {
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

impl fmt::Debug for Address {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", hex::encode(&self.0))
	}
}

impl fmt::Display for Address {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", hex::encode(&self.0))
	}
}

impl Address {
	pub fn from_hex(hex: &str) -> CommonResult<Self> {
		let hex =
			hex::decode(hex).map_err(|e| CommonError::new(CommonErrorKind::Codec, Box::new(e)))?;
		Ok(Address(hex))
	}
}

impl<'a> TransactionForHash<'a> {
	pub fn new(tx: &'a Transaction) -> Self {
		let witness = tx
			.witness
			.as_ref()
			.map(|witness| (&witness.public_key, &witness.nonce, &witness.until));
		let call = &tx.call;
		Self { witness, call }
	}
}

impl Event {
	pub fn from<E: Encode>(data: &E) -> CommonResult<Self> {
		let vec = codec::encode(data)?;
		Ok(Self(vec))
	}
}
