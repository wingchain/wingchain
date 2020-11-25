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

#[derive(Clone)]
pub struct Hash(pub Vec<u8>);

#[derive(Clone)]
pub struct Address(pub Vec<u8>);

pub type BlockNumber = u64;

pub type Balance = u64;

pub type ContractResult<T> = Result<T, ContractError>;

#[derive(Serialize)]
pub struct ContractEvent<T: Serialize> {
	pub name: String,
	pub data: T,
}

#[derive(Debug, Clone)]
pub enum ContractError {
	Serialize,
	Deserialize,
	InvalidMethod,
	InvalidParams,
	InvalidAddress,
	BadUTF8,
	ShareIllegalAccess,
	ShareValueLenExceeded,
	ShareSizeExceeded,
	Transfer,
	NotPayable,
	Panic { msg: String },
	User { msg: String },
}

#[derive(Serialize, Deserialize)]
pub struct EmptyParams;

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
		formatter.write_str("address")
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

impl<'de> de::Deserialize<'de> for Address {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: de::Deserializer<'de>,
	{
		deserializer.deserialize_string(AddressVisitor)
	}
}

impl fmt::Display for ContractError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ContractError::Serialize => write!(f, "Serialize"),
			ContractError::Deserialize => write!(f, "Deserialize"),
			ContractError::InvalidMethod => write!(f, "InvalidMethod"),
			ContractError::InvalidParams => write!(f, "InvalidParams"),
			ContractError::InvalidAddress => write!(f, "InvalidAddress"),
			ContractError::BadUTF8 => write!(f, "BadUTF8"),
			ContractError::ShareIllegalAccess => write!(f, "ShareIllegalAccess"),
			ContractError::ShareSizeExceeded => write!(f, "ShareSizeExceeded"),
			ContractError::ShareValueLenExceeded => write!(f, "ShareValueLenExceeded"),
			ContractError::Transfer => write!(f, "Transfer"),
			ContractError::NotPayable => write!(f, "NotPayable"),
			ContractError::Panic { msg } => write!(f, "Panic: {}", msg),
			ContractError::User { msg } => write!(f, "{}", msg),
		}
	}
}

impl From<&str> for ContractError {
	fn from(v: &str) -> Self {
		match v {
			"Serialize" => ContractError::Serialize,
			"Deserialize" => ContractError::Deserialize,
			"InvalidMethod" => ContractError::InvalidMethod,
			"InvalidParams" => ContractError::InvalidParams,
			"InvalidAddress" => ContractError::InvalidAddress,
			"BadUTF8" => ContractError::BadUTF8,
			"ShareIllegalAccess" => ContractError::ShareIllegalAccess,
			"ShareSizeExceeded" => ContractError::ShareSizeExceeded,
			"ShareValueLenExceeded" => ContractError::ShareValueLenExceeded,
			"Transfer" => ContractError::Transfer,
			"NotPayable" => ContractError::NotPayable,
			_ => {
				let split = v.find(": ").map(|p| (&v[..p], &v[p + 1..]));
				match split {
					Some((prefix, suffix)) => match prefix {
						"Panic" => ContractError::Panic {
							msg: suffix.to_string(),
						},
						_ => ContractError::User { msg: v.to_string() },
					},
					_ => ContractError::User { msg: v.to_string() },
				}
			}
		}
	}
}
