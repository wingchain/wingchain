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

use futures::channel::oneshot;
use jsonrpc_v2::{Data, ErrorLike, Params};
use serde::{Deserialize, Serialize};

use primitives::errors::Display;
use primitives::errors::{CommonError, CommonResult};
use primitives::{codec, SecretKey};

use crate::errors;
use crate::errors::ErrorKind;
use crate::support::ApiSupport;
use node_coordinator::{CoordinatorInMessage, NetworkInMessage};
use std::collections::HashSet;

pub async fn chain_get_header_by_number<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((block_number,)): Params<(BlockNumber,)>,
) -> CustomResult<Option<Header>> {
	let number_enum: BlockNumberEnum = block_number.try_into()?;

	let support = data.0;
	let number = match number_enum {
		BlockNumberEnum::Confirmed => support.get_confirmed_number()?,
		BlockNumberEnum::ConfirmedExecuted => support.get_confirmed_executed_number()?,
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
		BlockNumberEnum::Confirmed => support.get_confirmed_number()?,
		BlockNumberEnum::ConfirmedExecuted => support.get_confirmed_executed_number()?,
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

pub async fn chain_get_proof_by_hash<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((hash,)): Params<(Hash,)>,
) -> CustomResult<Option<Proof>> {
	let hash = hash.try_into()?;
	let support = data.0;
	let proof: Option<Proof> = support.get_proof(&hash)?.map(Into::into);

	let proof = proof.map(|mut x| {
		x.hash = Some(hash.into());
		x
	});

	Ok(proof)
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

pub async fn chain_get_receipt_by_hash<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((hash,)): Params<(Hash,)>,
) -> CustomResult<Option<Receipt>> {
	let hash = hash.try_into()?;
	let support = data.0;
	let tx: Option<Receipt> = support.get_receipt(&hash)?.map(Into::into);

	let tx = tx.map(|mut x| {
		x.hash = Some(hash.into());
		x
	});

	Ok(tx)
}

pub async fn chain_send_raw_transaction<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((raw_transaction,)): Params<(Hex,)>,
) -> CustomResult<Hash> {
	let raw_transaction: Vec<u8> = raw_transaction.try_into()?;
	let transaction: CommonResult<primitives::Transaction> = codec::decode(&raw_transaction)
		.map_err(|_| {
			errors::ErrorKind::InvalidParams("Invalid raw transaction".to_string()).into()
		});
	let transaction = transaction?;

	let support = data.0;

	let tx_hash = support.hash_transaction(&transaction)?.into();

	support.insert_transaction(transaction)?;

	Ok(tx_hash)
}

pub async fn chain_execute_call<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params(request): Params<ExecuteTransactionRequest>,
) -> CustomResult<Hex> {
	let block_hash: primitives::Hash = request.block_hash.try_into()?;
	let sender: Option<primitives::Address> = match request.sender {
		Some(sender) => Some(sender.try_into()?),
		None => None,
	};
	let call: primitives::Call = request.call.try_into()?;

	let result = data.execute_call(&block_hash, sender.as_ref(), &call)?;

	let result: CommonResult<Vec<u8>> = result.map_err(|e| errors::ErrorKind::CallError(e).into());
	let result = result?;

	let result = result.into();

	Ok(result)
}

pub async fn chain_build_transaction<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params(request): Params<BuildTransactionRequest>,
) -> CustomResult<Hex> {
	let witness = match request.witness {
		Some((secret_key, nonce, until)) => {
			let secret_key: Vec<u8> = secret_key.try_into()?;
			let nonce: u32 = nonce.try_into()?;
			let until: u64 = until.try_into()?;
			Some((SecretKey(secret_key), nonce, until))
		}
		None => None,
	};
	let call: primitives::Call = request.call.try_into()?;
	let result = data.build_transaction(witness, call)?;
	let result = codec::encode(&result)?.into();
	Ok(result)
}

pub async fn txpool_get_transaction<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params((hash,)): Params<(Hash,)>,
) -> CustomResult<Option<Transaction>> {
	let hash = hash.try_into()?;
	let support = data.0;

	let tx: Option<Transaction> = support.txpool_get_transaction(&hash)?.map(Into::into);

	let tx = tx.map(|mut x| {
		x.hash = Some(hash.into());
		x
	});

	Ok(tx)
}

pub async fn network_get_state<S: ApiSupport>(
	data: Data<Arc<S>>,
	Params(_request): Params<EmptyRequest>,
) -> CustomResult<NetworkState> {
	let co_tx = data.0.coordinator_tx()?;
	let (tx, rx) = oneshot::channel();
	co_tx
		.unbounded_send(CoordinatorInMessage::Network(
			NetworkInMessage::GetNetworkState { tx },
		))
		.map_err(|e| CommonError::from(ErrorKind::CallError(format!("{}", e))))?;
	let network_state = rx
		.await
		.map_err(|e| CommonError::from(ErrorKind::CallError(format!("{}", e))))?;
	let network_state = network_state.into();
	Ok(network_state)
}

/// Number input: number, hex or tag (confirmed, confirmed_executed)
#[derive(Deserialize)]
#[serde(untagged)]
pub enum BlockNumber {
	Number(primitives::types::BlockNumber),
	String(String),
}

/// Number input: number or hex
#[derive(Deserialize)]
#[serde(untagged)]
pub enum NumberOrHex {
	Number(u64),
	String(String),
}

/// Hash
#[derive(Serialize, Deserialize, Clone)]
pub struct Hash(String);

/// Address
#[derive(Serialize, Deserialize)]
pub struct Address(String);

/// Hex format for number, private key, public key, signature, params
#[derive(Serialize, Deserialize)]
pub struct Hex(String);

enum BlockNumberEnum {
	Number(primitives::BlockNumber),
	Confirmed,
	ConfirmedExecuted,
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
	pub meta_receipts_root: Hash,
	pub payload_txs_root: Hash,
	pub payload_execution_gap: Hex,
	pub payload_execution_state_root: Hash,
	pub payload_execution_receipts_root: Hash,
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
pub struct Proof {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hash: Option<Hash>,
	pub name: String,
	pub data: Hex,
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
	public_key: Hex,
	signature: Hex,
	nonce: Hex,
	until: Hex,
}

#[derive(Serialize, Deserialize)]
pub struct Call {
	pub module: String,
	pub method: String,
	pub params: Hex,
}

#[derive(Serialize)]
pub struct Receipt {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hash: Option<Hash>,
	pub block_number: Hex,
	pub events: Vec<serde_json::Value>,
	pub result: Result<Hex, String>,
}

#[derive(Deserialize)]
pub struct ExecuteTransactionRequest {
	pub block_hash: Hash,
	pub sender: Option<Address>,
	pub call: Call,
}

#[derive(Deserialize)]
pub struct BuildTransactionRequest {
	pub witness: Option<(Hex, NumberOrHex, NumberOrHex)>,
	pub call: Call,
}

#[derive(Deserialize)]
pub struct EmptyRequest {}

#[derive(Serialize)]
pub struct NetworkState {
	pub peer_id: String,
	pub listened_addresses: HashSet<String>,
	pub external_addresses: HashSet<String>,
	pub opened_peers: Vec<OpenedPeer>,
	pub unopened_peers: Vec<UnopenedPeer>,
}

#[derive(Serialize)]
pub struct OpenedPeer {
	peer_id: String,
	connected_point: String,
	known_addresses: HashSet<String>,
	agent_version: Option<String>,
	latest_ping: Option<String>,
}

#[derive(Serialize)]
pub struct UnopenedPeer {
	peer_id: String,
	known_addresses: HashSet<String>,
}

impl From<node_coordinator::NetworkState> for NetworkState {
	fn from(v: node_coordinator::NetworkState) -> Self {
		Self {
			peer_id: v.peer_id.to_string(),
			listened_addresses: v
				.listened_addresses
				.into_iter()
				.map(|v| v.to_string())
				.collect(),
			external_addresses: v
				.external_addresses
				.into_iter()
				.map(|v| v.to_string())
				.collect(),
			opened_peers: v.opened_peers.into_iter().map(Into::into).collect(),
			unopened_peers: v.unopened_peers.into_iter().map(Into::into).collect(),
		}
	}
}

impl From<node_coordinator::OpenedPeer> for OpenedPeer {
	fn from(v: node_coordinator::OpenedPeer) -> Self {
		Self {
			peer_id: v.peer_id.to_string(),
			connected_point: format!("{:?}", v.connected_point),
			known_addresses: v
				.known_addresses
				.into_iter()
				.map(|v| v.to_string())
				.collect(),
			agent_version: v.agent_version,
			latest_ping: v.latest_ping.map(|x| format!("{:?}", x)),
		}
	}
}

impl From<node_coordinator::UnopenedPeer> for UnopenedPeer {
	fn from(v: node_coordinator::UnopenedPeer) -> Self {
		Self {
			peer_id: v.peer_id.to_string(),
			known_addresses: v
				.known_addresses
				.into_iter()
				.map(|v| v.to_string())
				.collect(),
		}
	}
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
			meta_receipts_root: header.meta_receipts_root.into(),
			payload_txs_root: header.payload_txs_root.into(),
			payload_execution_gap: header.payload_execution_gap.into(),
			payload_execution_state_root: header.payload_execution_state_root.into(),
			payload_execution_receipts_root: header.payload_execution_receipts_root.into(),
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
			public_key: witness.public_key.into(),
			signature: witness.signature.into(),
			nonce: witness.nonce.into(),
			until: witness.until.into(),
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

impl From<primitives::Receipt> for Receipt {
	fn from(receipt: primitives::Receipt) -> Self {
		Self {
			hash: None,
			block_number: receipt.block_number.into(),
			events: receipt
				.events
				.into_iter()
				.map(|x| serde_json::from_slice(&x.0).unwrap_or(serde_json::Value::Null))
				.collect(),
			result: receipt.result.map(Into::into),
		}
	}
}

impl From<primitives::Proof> for Proof {
	fn from(proof: primitives::Proof) -> Self {
		Self {
			hash: None,
			name: proof.name,
			data: proof.data.into(),
		}
	}
}

impl From<u32> for Hex {
	fn from(number: u32) -> Self {
		Hex(format!("0x{}", hex::encode(number.to_be_bytes())))
	}
}

impl From<u64> for Hex {
	fn from(number: u64) -> Self {
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

impl From<primitives::PublicKey> for Hex {
	fn from(public_key: primitives::PublicKey) -> Self {
		Hex(format!("0x{}", hex::encode(public_key.0)))
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

impl From<primitives::Event> for Hex {
	fn from(event: primitives::Event) -> Self {
		Hex(format!("0x{}", hex::encode(event.0)))
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
			.map_err(|_| errors::ErrorKind::InvalidParams(format!("Invalid hex: {}", hex)))?;
		Ok(primitives::Hash(hex))
	}
}

impl TryInto<primitives::Address> for Address {
	type Error = CommonError;

	fn try_into(self) -> Result<primitives::Address, Self::Error> {
		let hex = self.0.trim_start_matches("0x");
		let hex = hex::decode(hex)
			.map_err(|_| errors::ErrorKind::InvalidParams(format!("Invalid hex: {}", hex)))?;
		Ok(primitives::Address(hex))
	}
}

impl TryInto<Vec<u8>> for Hex {
	type Error = CommonError;

	fn try_into(self) -> Result<Vec<u8>, Self::Error> {
		let hex = self.0.trim_start_matches("0x");
		let hex = hex::decode(hex)
			.map_err(|_| errors::ErrorKind::InvalidParams(format!("Invalid hex: {}", hex)))?;
		Ok(hex)
	}
}

impl TryInto<u64> for Hex {
	type Error = CommonError;
	fn try_into(self) -> Result<u64, Self::Error> {
		let number = self.0;
		let number = if number.starts_with("0x") {
			let hex = number.trim_start_matches("0x");
			u64::from_str_radix(hex, 16)
				.map_err(|_| errors::ErrorKind::InvalidParams(format!("Invalid hex: {}", number)))?
		} else {
			number.parse::<u64>().map_err(|_| {
				errors::ErrorKind::InvalidParams(format!("Invalid number: {}", number))
			})?
		};
		Ok(number)
	}
}

impl TryInto<u32> for Hex {
	type Error = CommonError;
	fn try_into(self) -> Result<u32, Self::Error> {
		let number = self.0;
		let number = if number.starts_with("0x") {
			let hex = number.trim_start_matches("0x");
			u32::from_str_radix(hex, 16)
				.map_err(|_| errors::ErrorKind::InvalidParams(format!("Invalid hex: {}", number)))?
		} else {
			number.parse::<u32>().map_err(|_| {
				errors::ErrorKind::InvalidParams(format!("Invalid number: {}", number))
			})?
		};
		Ok(number)
	}
}

impl TryInto<primitives::Call> for Call {
	type Error = CommonError;

	fn try_into(self) -> Result<primitives::Call, Self::Error> {
		let params = primitives::Params(self.params.try_into()?);

		Ok(primitives::Call {
			module: self.module,
			method: self.method,
			params,
		})
	}
}

impl TryFrom<BlockNumber> for BlockNumberEnum {
	type Error = CommonError;

	fn try_from(value: BlockNumber) -> Result<Self, Self::Error> {
		match value {
			BlockNumber::Number(number) => Ok(BlockNumberEnum::Number(number)),
			BlockNumber::String(str) => {
				let result = match str.as_str() {
					"confirmed" => BlockNumberEnum::Confirmed,
					"confirmed_executed" => BlockNumberEnum::ConfirmedExecuted,
					number if number.starts_with("0x") => {
						let hex = number.trim_start_matches("0x");
						let number = u64::from_str_radix(hex, 16).map_err(|_| {
							errors::ErrorKind::InvalidParams(format!("Invalid hex: {}", number))
						})?;
						BlockNumberEnum::Number(number)
					}
					number => {
						let number = number.parse::<u64>().map_err(|_| {
							errors::ErrorKind::InvalidParams(format!("Invalid number: {}", number))
						})?;
						BlockNumberEnum::Number(number)
					}
				};
				Ok(result)
			}
		}
	}
}

impl TryInto<u64> for NumberOrHex {
	type Error = CommonError;
	fn try_into(self) -> Result<u64, Self::Error> {
		match self {
			NumberOrHex::Number(number) => Ok(number),
			NumberOrHex::String(str) => {
				let result = match str.as_str() {
					number if number.starts_with("0x") => {
						let hex = number.trim_start_matches("0x");
						u64::from_str_radix(hex, 16).map_err(|_| {
							errors::ErrorKind::InvalidParams(format!("Invalid hex: {}", number))
						})?
					}
					number => number.parse::<u64>().map_err(|_| {
						errors::ErrorKind::InvalidParams(format!("Invalid number: {}", number))
					})?,
				};
				Ok(result)
			}
		}
	}
}

impl TryInto<u32> for NumberOrHex {
	type Error = CommonError;
	fn try_into(self) -> Result<u32, Self::Error> {
		let number: u64 = self.try_into()?;
		let number: u32 = number
			.try_into()
			.map_err(|_| errors::ErrorKind::InvalidParams(format!("Invalid number: {}", number)))?;
		Ok(number)
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
		-32000
	}

	fn message(&self) -> String {
		"Server error".to_string()
	}

	fn data(&self) -> Option<BoxedSerialize> {
		Some(Box::new(self.0.to_string()))
	}
}
