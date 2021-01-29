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
use primitives::{PublicKey, Signature};
use utils_enum_codec::enum_codec;

#[enum_codec]
#[derive(From, TryInto)]
pub enum RaftMessage {
	RegisterValidatorReq(RegisterValidatorReq),
	RegisterValidatorRes(RegisterValidatorRes),
}

#[derive(Encode, Decode)]
pub struct RegisterValidatorReq {
	pub request_id: RequestId,
	pub public_key: PublicKey,
	pub signature: Signature,
}

#[derive(Encode, Decode)]
pub struct RegisterValidatorRes {
	pub request_id: RequestId,
	pub success: bool,
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

pub trait RequestIdAware {
	fn get_request_id(&self) -> RequestId;
	fn set_request_id(&mut self, request_id: RequestId);
}

#[derive(Encode, Decode, Debug, PartialEq, Clone, Display, Hash, Eq)]
pub struct RequestId(pub u64);
