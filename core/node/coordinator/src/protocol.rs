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

use primitives::codec::{Decode, Encode};
use primitives::types::FullHeader;
use primitives::Hash;

#[derive(Debug, PartialEq)]
pub enum ProtocolMessage {
	Handshake(Handshake),
	BlockAnnounce(BlockAnnounce),
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct Handshake {
	pub genesis_hash: Hash,
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub struct BlockAnnounce {
	pub header: FullHeader,
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
			_ => Err("unknown protocol message name".into()),
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
		});
		let encoded = message.encode();
		assert_eq!(
			encoded,
			vec![36, 72, 97, 110, 100, 115, 104, 97, 107, 101, 16, 12, 1, 2, 3]
		);
	}

	#[test]
	fn test_decode() {
		let encoded = vec![
			36u8, 72, 97, 110, 100, 115, 104, 97, 107, 101, 16, 12, 1, 2, 3,
		];
		let message: ProtocolMessage = Decode::decode(&mut &encoded[..]).unwrap();
		assert_eq!(
			message,
			ProtocolMessage::Handshake(Handshake {
				genesis_hash: Hash(vec![1, 2, 3]),
			})
		)
	}
}
