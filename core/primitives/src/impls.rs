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

use serde::de::{Error, Unexpected, Visitor};
use serde::{de, ser, Deserialize, Serialize};

use crate::errors::{CommonError, CommonErrorKind, CommonResult};
use crate::types::{Address, Event, Hash, Transaction, TransactionForHash};
use crate::Proof;

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

impl ser::Serialize for Address {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: ser::Serializer,
	{
		serializer.serialize_str(hex::encode(&self.0).as_str())
	}
}

struct AddressVisitor;

impl<'de> Visitor<'de> for AddressVisitor {
	type Value = Address;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("Address")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: Error,
	{
		let result =
			hex::decode(&v).map_err(|_| Error::invalid_value(Unexpected::Str(&v), &self))?;
		Ok(Address(result))
	}

	fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
	where
		E: Error,
	{
		self.visit_str(&v)
	}

	fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
	where
		E: Error,
	{
		let v = std::str::from_utf8(v)
			.map_err(|_| Error::invalid_value(Unexpected::Bytes(&v), &self))?;
		self.visit_str(v)
	}

	fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
	where
		E: Error,
	{
		self.visit_bytes(&v)
	}
}

impl<'de> Deserialize<'de> for Address {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: de::Deserializer<'de>,
	{
		deserializer.deserialize_string(AddressVisitor)
	}
}

impl ser::Serialize for Hash {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: ser::Serializer,
	{
		serializer.serialize_str(hex::encode(&self.0).as_str())
	}
}

struct HashVisitor;

impl<'de> Visitor<'de> for HashVisitor {
	type Value = Hash;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("Hash")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: Error,
	{
		let result =
			hex::decode(&v).map_err(|_| Error::invalid_value(Unexpected::Str(&v), &self))?;
		Ok(Hash(result))
	}

	fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
	where
		E: Error,
	{
		self.visit_str(&v)
	}

	fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
	where
		E: Error,
	{
		let v = std::str::from_utf8(v)
			.map_err(|_| Error::invalid_value(Unexpected::Bytes(&v), &self))?;
		self.visit_str(v)
	}

	fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
	where
		E: Error,
	{
		self.visit_bytes(&v)
	}
}

impl<'de> Deserialize<'de> for Hash {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: de::Deserializer<'de>,
	{
		deserializer.deserialize_string(HashVisitor)
	}
}

#[derive(Serialize)]
struct EventType<S: Serialize> {
	name: String,
	data: S,
}

impl Event {
	pub fn from_raw(raw: Vec<u8>) -> Self {
		Self(raw)
	}
	pub fn from_data<S: Serialize>(name: String, data: S) -> CommonResult<Self> {
		let event = EventType { name, data };
		let vec = serde_json::to_vec(&event)
			.map_err(|e| CommonError::new(CommonErrorKind::Codec, Box::new(e)))?;
		Ok(Self(vec))
	}
}

impl Default for Proof {
	fn default() -> Self {
		Proof {
			name: "".to_string(),
			data: vec![],
		}
	}
}
