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
use primitives::{BlockNumber, Hash, PublicKey, Signature, Transaction};
use utils_enum_codec::enum_codec;

#[enum_codec]
#[derive(From, TryInto)]
pub enum RaftMessage {
	RegisterValidatorReq(RegisterValidatorReq),
	RegisterValidatorRes(RegisterValidatorRes),
	AppendEntriesReq(AppendEntriesReq),
	AppendEntriesRes(AppendEntriesRes),
	RequestVoteReq(RequestVoteReq),
	RequestVoteRes(RequestVoteRes),
	RequestProposalReq(RequestProposalReq),
	RequestProposalRes(RequestProposalRes),
}

#[derive(Encode, Decode, Debug)]
pub struct RegisterValidatorReq {
	pub request_id: RequestId,
	pub public_key: PublicKey,
	pub signature: Signature,
}

#[derive(Encode, Decode, Debug)]
pub struct RegisterValidatorRes {
	pub request_id: RequestId,
	pub success: bool,
}

#[derive(Encode, Decode, Debug)]
pub struct AppendEntriesReq {
	pub request_id: RequestId,
	pub term: u64,
	pub prev_log_index: u64,
	pub prev_log_term: u64,
	pub commit_log_index: u64,
	pub entries: Vec<Entry>,
}

#[derive(Encode, Decode, Debug)]
pub struct AppendEntriesRes {
	pub request_id: RequestId,
	pub success: bool,
	pub term: u64,
	pub last_log_index: u64,
	pub last_log_term: u64,
}

#[derive(Encode, Decode, Debug)]
pub struct RequestVoteReq {
	pub request_id: RequestId,
	pub term: u64,
	pub last_log_index: u64,
	pub last_log_term: u64,
}

#[derive(Encode, Decode, Debug)]
pub struct RequestVoteRes {
	pub request_id: RequestId,
	pub term: u64,
	pub vote_granted: bool,
}

#[derive(Encode, Decode, Debug)]
pub struct RequestProposalReq {
	pub request_id: RequestId,
}

#[derive(Encode, Decode, Debug)]
pub struct RequestProposalRes {
	pub request_id: RequestId,
	pub extra_election_timeout: u64,
	pub proposal: Option<Proposal>,
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct Entry {
	pub term: u64,
	pub index: u64,
	pub data: EntryData,
}

#[derive(Encode, Decode, Debug, Clone)]
pub enum EntryData {
	Blank,
	Proposal { block_hash: Hash },
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct Proposal {
	pub block_hash: Hash,
	pub number: BlockNumber,
	pub timestamp: u64,
	pub meta_txs: Vec<Transaction>,
	pub payload_txs: Vec<Transaction>,
	pub execution_number: BlockNumber,
}

impl RequestIdAware for RegisterValidatorReq {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

impl RequestIdAware for RegisterValidatorRes {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

impl RequestIdAware for AppendEntriesReq {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

impl RequestIdAware for AppendEntriesRes {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

impl RequestIdAware for RequestVoteReq {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

impl RequestIdAware for RequestVoteRes {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

impl RequestIdAware for RequestProposalReq {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

impl RequestIdAware for RequestProposalRes {
	fn get_request_id(&self) -> RequestId {
		self.request_id.clone()
	}
	fn set_request_id(&mut self, request_id: RequestId) {
		self.request_id = request_id;
	}
}

pub trait RequestIdAware {
	fn get_request_id(&self) -> RequestId;
	fn set_request_id(&mut self, request_id: RequestId);
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Display, Hash, Eq)]
pub struct RequestId(pub u64);
