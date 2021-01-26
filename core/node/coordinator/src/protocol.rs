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

use derive_more::Display;
use std::convert::{TryFrom, TryInto};

use primitives::codec::{Decode, Encode};
use primitives::{BlockNumber, Hash, Header, Proof, Transaction};

#[derive(Debug, PartialEq)]
pub enum ProtocolMessage {
	Handshake(Handshake),
	BlockAnnounce(BlockAnnounce),
	BlockRequest(BlockRequest),
	BlockResponse(BlockResponse),
	TxPropagate(TxPropagate),
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Handshake {
	pub genesis_hash: Hash,
	pub confirmed_number: BlockNumber,
	pub confirmed_hash: Hash,
	pub nonce: u64,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct BlockAnnounce {
	pub block_hash: Hash,
	pub header: Header,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct BlockRequest {
	pub request_id: RequestId,
	pub fields: Fields,
	pub block_id: BlockId,
	pub count: u32,
	pub direction: Direction,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct BlockResponse {
	pub request_id: RequestId,
	pub blocks: Vec<BlockData>,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct TxPropagate {
	pub txs: Vec<Transaction>,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Display)]
pub struct RequestId(pub u64);

pub type Fields = u32;
pub const FIELDS_HEADER: u32 = 0b0001;
pub const FIELDS_BODY: u32 = 0b0010;
pub const FIELDS_PROOF: u32 = 0b0100;

#[derive(Encode, Decode, Debug, PartialEq)]
pub enum BlockId {
	Number(BlockNumber),
	Hash(Hash),
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub enum Direction {
	Asc,
	Desc,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct BlockData {
	pub number: BlockNumber,
	pub block_hash: Hash,
	pub header: Option<Header>,
	pub body: Option<BodyData>,
	pub proof: Option<Proof>,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct BodyData {
	pub meta_txs: Vec<Transaction>,
	pub payload_txs: Vec<Transaction>,
}

impl Encode for ProtocolMessage {
	fn encode(&self) -> Vec<u8> {
		let payload: ProtocolMessagePayload = self.into();
		payload.encode()
	}
}

impl Decode for ProtocolMessage {
	fn decode<I: scale_codec::Input>(value: &mut I) -> Result<Self, scale_codec::Error> {
		let payload: ProtocolMessagePayload = Decode::decode(value)?;
		payload.try_into()
	}
}

/// The equivalent of ProtocolMessage, which has a
/// more compatible codec result than enum type
#[derive(Encode, Decode)]
struct ProtocolMessagePayload {
	name: String,
	payload: Vec<u8>,
}

impl<'a> From<&'a ProtocolMessage> for ProtocolMessagePayload {
	fn from(v: &'a ProtocolMessage) -> Self {
		match v {
			ProtocolMessage::Handshake(v) => ProtocolMessagePayload {
				name: "Handshake".to_string(),
				payload: v.encode(),
			},
			ProtocolMessage::BlockAnnounce(v) => ProtocolMessagePayload {
				name: "BlockAnnounce".to_string(),
				payload: v.encode(),
			},
			ProtocolMessage::BlockRequest(v) => ProtocolMessagePayload {
				name: "BlockRequest".to_string(),
				payload: v.encode(),
			},
			ProtocolMessage::BlockResponse(v) => ProtocolMessagePayload {
				name: "BlockResponse".to_string(),
				payload: v.encode(),
			},
			ProtocolMessage::TxPropagate(v) => ProtocolMessagePayload {
				name: "TxPropagate".to_string(),
				payload: v.encode(),
			},
		}
	}
}

impl TryFrom<ProtocolMessagePayload> for ProtocolMessage {
	type Error = scale_codec::Error;
	fn try_from(v: ProtocolMessagePayload) -> Result<Self, Self::Error> {
		match v.name.as_str() {
			"Handshake" => Ok(ProtocolMessage::Handshake(Decode::decode(
				&mut &v.payload[..],
			)?)),
			"BlockAnnounce" => Ok(ProtocolMessage::BlockAnnounce(Decode::decode(
				&mut &v.payload[..],
			)?)),
			"BlockRequest" => Ok(ProtocolMessage::BlockRequest(Decode::decode(
				&mut &v.payload[..],
			)?)),
			"BlockResponse" => Ok(ProtocolMessage::BlockResponse(Decode::decode(
				&mut &v.payload[..],
			)?)),
			"TxPropagate" => Ok(ProtocolMessage::TxPropagate(Decode::decode(
				&mut &v.payload[..],
			)?)),
			_ => Err("Unknown protocol message name".into()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encode() {
		let message = ProtocolMessage::Handshake(Handshake {
			genesis_hash: Hash(vec![1, 2, 3]),
			confirmed_number: 1,
			confirmed_hash: Hash(vec![4, 5, 6]),
			nonce: 2,
		});
		let encoded = message.encode();
		assert_eq!(
			encoded,
			vec![
				36, 72, 97, 110, 100, 115, 104, 97, 107, 101, 96, 12, 1, 2, 3, 1, 0, 0, 0, 0, 0, 0,
				0, 12, 4, 5, 6, 2, 0, 0, 0, 0, 0, 0, 0
			]
		);
	}

	#[test]
	fn test_decode() {
		let encoded = vec![
			36, 72, 97, 110, 100, 115, 104, 97, 107, 101, 96, 12, 1, 2, 3, 1, 0, 0, 0, 0, 0, 0, 0,
			12, 4, 5, 6, 2, 0, 0, 0, 0, 0, 0, 0,
		];
		let message: ProtocolMessage = Decode::decode(&mut &encoded[..]).unwrap();
		assert_eq!(
			message,
			ProtocolMessage::Handshake(Handshake {
				genesis_hash: Hash(vec![1, 2, 3]),
				confirmed_number: 1,
				confirmed_hash: Hash(vec![4, 5, 6]),
				nonce: 2,
			})
		)
	}
}
