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

use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use jsonrpc_v2::{Data, ErrorLike, Params};
use serde::{Deserialize, Serialize};

use primitives::errors::CommonError;
use primitives::errors::Display;

use crate::errors;
use crate::support::ApiSupport;

pub async fn chain_get_header_by_number<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((block_number,)): Params<(BlockNumber,)>,
) -> CustomResult<Option<Header>> {
	let number_enum: BlockNumberEnum = block_number.try_into()?;

	let support = data.0;
	let number = match number_enum {
		BlockNumberEnum::Best => support.get_best_number()?,
		BlockNumberEnum::Executed => support.get_executed_number()?,
		BlockNumberEnum::Number(number) => Some(number),
	};

	let number = match number {
		Some(number) => number,
		None => return Ok(None),
	};

	let block_hash = match support.get_block_hash(&number)? {
		Some(block_hash) => block_hash,
		None => return Ok(None),
	};

	let header: Option<Header> = support.get_header(&block_hash)?.map(Into::into);

	let header = header.map(|mut x| {
		x.hash = Some(block_hash.into());
		x
	});

	Ok(header)
}

pub async fn chain_get_header_by_hash<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((hash,)): Params<(Hash,)>,
) -> CustomResult<Option<Header>> {
	let hash = hash.try_into()?;
	let support = data.0;
	let header: Option<Header> = support.get_header(&hash)?.map(Into::into);

	let header = header.map(|mut x| {
		x.hash = Some(hash.into());
		x
	});

	Ok(header)
}

pub async fn chain_get_block_by_number<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((block_number,)): Params<(BlockNumber,)>,
) -> CustomResult<Option<Block>> {
	let number_enum: BlockNumberEnum = block_number.try_into()?;

	let support = data.0;
	let number = match number_enum {
		BlockNumberEnum::Best => support.get_best_number()?,
		BlockNumberEnum::Executed => support.get_executed_number()?,
		BlockNumberEnum::Number(number) => Some(number),
	};

	let number = match number {
		Some(number) => number,
		None => return Ok(None),
	};

	let block_hash = match support.get_block_hash(&number)? {
		Some(block_hash) => block_hash,
		None => return Ok(None),
	};

	let block: Option<Block> = support.get_block(&block_hash)?.map(Into::into);

	let block = block.map(|mut x| {
		x.hash = Some(block_hash.into());
		x
	});

	Ok(block)
}

pub async fn chain_get_block_by_hash<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((hash,)): Params<(Hash,)>,
) -> CustomResult<Option<Block>> {
	let hash = hash.try_into()?;
	let support = data.0;
	let block: Option<Block> = support.get_block(&hash)?.map(Into::into);

	let block = block.map(|mut x| {
		x.hash = Some(hash.into());
		x
	});

	Ok(block)
}

pub async fn chain_get_transaction_by_hash<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((hash,)): Params<(Hash,)>,
) -> CustomResult<Option<Transaction>> {
	let hash = hash.try_into()?;
	let support = data.0;
	let tx: Option<Transaction> = support.get_transaction(&hash)?.map(Into::into);

	let tx = tx.map(|mut x| {
		x.hash = Some(hash.into());
		x
	});

	Ok(tx)
}

pub async fn chain_get_raw_transaction_by_hash<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((hash,)): Params<(Hash,)>,
) -> CustomResult<Option<Hex>> {
	let hash = hash.try_into()?;
	let support = data.0;
	let raw_tx: Option<Hex> = support.get_raw_transaction(&hash)?.map(Into::into);
	Ok(raw_tx)
}

/// Number input: number, hex or tag (best, executed)
#[derive(Deserialize)]
pub struct BlockNumber(String);

/// Hash
#[derive(Serialize, Deserialize, Clone)]
pub struct Hash(String);

/// Hex format for number
#[derive(Serialize)]
pub struct Hex(String);

enum BlockNumberEnum {
	Number(primitives::BlockNumber),
	Best,
	Executed,
}

#[derive(Serialize)]
pub struct Header {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hash: Option<Hash>,
	pub number: Hex,
	pub timestamp: Hex,
	pub parent_hash: Hash,
	pub meta_txs_root: Hash,
	pub meta_state_root: Hash,
	pub payload_txs_root: Hash,
	pub payload_executed_gap: Hex,
	pub payload_executed_state_root: Hash,
}

#[derive(Serialize)]
pub struct Block {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hash: Option<Hash>,
	pub header: Header,
	pub body: Body,
}

#[derive(Serialize)]
pub struct Body {
	pub meta_txs: Vec<Hash>,
	pub payload_txs: Vec<Hash>,
}

#[derive(Serialize)]
pub struct Transaction {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hash: Option<Hash>,
	pub witness: Option<Witness>,
	pub call: Call,
}

#[derive(Serialize)]
pub struct Witness {
	address: Hex,
	signature: Hex,
	nonce: Hex,
	expire: Hex,
}

#[derive(Serialize)]
pub struct Call {
	pub module: String,
	pub method: String,
	pub params: Hex,
}

impl From<primitives::Header> for Header {
	fn from(header: primitives::Header) -> Self {
		Self {
			hash: None,
			number: header.number.into(),
			timestamp: header.timestamp.into(),
			parent_hash: header.parent_hash.into(),
			meta_txs_root: header.meta_txs_root.into(),
			meta_state_root: header.meta_state_root.into(),
			payload_txs_root: header.payload_txs_root.into(),
			payload_executed_gap: header.payload_executed_gap.into(),
			payload_executed_state_root: header.payload_executed_state_root.into(),
		}
	}
}

impl From<primitives::Block> for Block {
	fn from(block: primitives::Block) -> Self {
		Self {
			hash: None,
			header: block.header.into(),
			body: block.body.into(),
		}
	}
}

impl From<primitives::Body> for Body {
	fn from(body: primitives::Body) -> Self {
		Self {
			meta_txs: body.meta_txs.into_iter().map(Into::into).collect(),
			payload_txs: body.payload_txs.into_iter().map(Into::into).collect(),
		}
	}
}

impl From<primitives::Transaction> for Transaction {
	fn from(transaction: primitives::Transaction) -> Self {
		Self {
			hash: None,
			witness: transaction.witness.map(Into::into),
			call: transaction.call.into(),
		}
	}
}

impl From<primitives::Witness> for Witness {
	fn from(witness: primitives::Witness) -> Self {
		Self {
			address: witness.address.into(),
			signature: witness.signature.into(),
			nonce: witness.nonce.into(),
			expire: witness.nonce.into(),
		}
	}
}

impl From<primitives::Call> for Call {
	fn from(call: primitives::Call) -> Self {
		Self {
			module: call.module,
			method: call.method,
			params: call.params.into(),
		}
	}
}

impl From<u32> for Hex {
	fn from(number: u32) -> Self {
		Hex(format!("0x{}", hex::encode(number.to_be_bytes())))
	}
}

impl From<i8> for Hex {
	fn from(number: i8) -> Self {
		Hex(format!("0x{}", hex::encode(number.to_be_bytes())))
	}
}

impl From<Vec<u8>> for Hex {
	fn from(vec: Vec<u8>) -> Self {
		Hex(format!("0x{}", hex::encode(vec)))
	}
}

impl From<primitives::Address> for Hex {
	fn from(address: primitives::Address) -> Self {
		Hex(format!("0x{}", hex::encode(address.0)))
	}
}

impl From<primitives::Signature> for Hex {
	fn from(signature: primitives::Signature) -> Self {
		Hex(format!("0x{}", hex::encode(signature.0)))
	}
}

impl From<primitives::Params> for Hex {
	fn from(params: primitives::Params) -> Self {
		Hex(format!("0x{}", hex::encode(params.0)))
	}
}

impl From<primitives::Hash> for Hash {
	fn from(hash: primitives::Hash) -> Self {
		Hash(format!("0x{}", hex::encode(hash.0)))
	}
}

impl TryInto<primitives::Hash> for Hash {
	type Error = CommonError;

	fn try_into(self) -> Result<primitives::Hash, Self::Error> {
		let hex = self.0.trim_start_matches("0x");
		let hex = hex::decode(hex)
			.map_err(|_| errors::ErrorKind::InvalidParams(format!("invalid hex: {}", hex)))?;
		Ok(primitives::Hash(hex))
	}
}

impl TryFrom<BlockNumber> for BlockNumberEnum {
	type Error = CommonError;

	fn try_from(value: BlockNumber) -> Result<Self, Self::Error> {
		let result = match value.0.as_str() {
			"best" => BlockNumberEnum::Best,
			"executed" => BlockNumberEnum::Executed,
			number if number.starts_with("0x") => {
				let hex = number.trim_start_matches("0x");
				let number = u32::from_str_radix(hex, 16).map_err(|_| {
					errors::ErrorKind::InvalidParams(format!("invalid hex: {}", number))
				})?;
				BlockNumberEnum::Number(number)
			}
			number => {
				let number = number.parse::<u32>().map_err(|_| {
					errors::ErrorKind::InvalidParams(format!("invalid number: {}", number))
				})?;
				BlockNumberEnum::Number(number)
			}
		};
		Ok(result)
	}
}

#[derive(Display)]
pub struct CustomError(CommonError);

pub type CustomResult<T> = Result<T, CustomError>;

impl From<CommonError> for CustomError {
	fn from(error: CommonError) -> Self {
		CustomError(error)
	}
}

type BoxedSerialize = Box<dyn erased_serde::Serialize + Send>;

impl ErrorLike for CustomError {
	fn code(&self) -> i64 {
		32000
	}

	fn message(&self) -> String {
		"Server error".to_string()
	}

	fn data(&self) -> Option<BoxedSerialize> {
		Some(Box::new(self.0.to_string()))
	}
}
