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

use derive_more::{Display, From, TryInto};
use primitives::codec::{Decode, Encode};
use primitives::{Address, BlockNumber, Hash, PublicKey, Signature, Transaction};
use utils_enum_codec::enum_codec;

// #[enum_codec]
#[derive(From, TryInto)]
pub enum HotStuffMessage {}

#[derive(Encode, Decode, Debug, Clone)]
pub enum MessageType {
	Prepare,
	PreCommit,
	Commit,
	Decide,
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct QC {
	pub message_type: MessageType,
	pub view: u64,
	pub node: Node,
	pub leader_address: Address,
	pub sig: Vec<(PublicKey, Signature)>,
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct Node {
	pub parent: Hash,
	pub block_hash: Hash,
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Display, Hash, Eq)]
pub struct RequestId(pub u64);
