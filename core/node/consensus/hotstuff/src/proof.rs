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

use crate::protocol::QC;
use crypto::dsa::{Dsa, DsaImpl, KeyPair};
use node_consensus_primitives::{CONSENSUS_HOTSTUFF, CONSENSUS_RAFT};
use primitives::codec::{self, Decode, Encode};
use primitives::errors::{CommonError, CommonResult};
use primitives::{Address, Hash, PublicKey, SecretKey, Signature};
use std::convert::TryFrom;
use std::sync::Arc;

#[derive(Encode, Decode, Debug)]
pub struct Proof {
	pub commit_qc: QC,
}

impl Proof {
	pub fn new(commit_qc: QC) -> CommonResult<Self> {
		Ok(Self { commit_qc })
	}
}

impl TryFrom<Proof> for primitives::Proof {
	type Error = CommonError;
	fn try_from(value: Proof) -> Result<Self, Self::Error> {
		Ok(Self {
			name: CONSENSUS_HOTSTUFF.to_string(),
			data: codec::encode(&value)?,
		})
	}
}
