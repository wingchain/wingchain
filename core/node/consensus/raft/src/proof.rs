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

use crypto::dsa::{Dsa, DsaImpl, KeyPair};
use node_consensus_primitives::CONSENSUS_RAFT;
use primitives::codec::{self, Decode, Encode};
use primitives::errors::{CommonError, CommonResult};
use primitives::{Hash, PublicKey, SecretKey, Signature};
use std::convert::TryFrom;
use std::sync::Arc;

#[derive(Encode, Decode, Debug)]
pub struct Proof {
	pub log_index: u64,
	pub log_term: u64,
	pub public_key: PublicKey,
	pub signature: Signature,
}

impl Proof {
	pub fn new(
		block_hash: &Hash,
		log_index: u64,
		log_term: u64,
		secret_key: &SecretKey,
		dsa: Arc<DsaImpl>,
	) -> CommonResult<Self> {
		let keypair = dsa.key_pair_from_secret_key(&secret_key.0)?;
		let (_, public_key_len, signature_len) = dsa.length().into();
		let public_key = {
			let mut out = vec![0u8; public_key_len];
			keypair.public_key(&mut out);
			PublicKey(out)
		};
		let signature = {
			let mut out = vec![0u8; signature_len];
			let message = (block_hash, log_index, log_term);
			let message = codec::encode(&message)?;
			keypair.sign(&message, &mut out);
			Signature(out)
		};
		Ok(Self {
			log_index,
			log_term,
			public_key,
			signature,
		})
	}
}

impl TryFrom<Proof> for primitives::Proof {
	type Error = CommonError;
	fn try_from(value: Proof) -> Result<Self, Self::Error> {
		Ok(Self {
			name: CONSENSUS_RAFT.to_string(),
			data: codec::encode(&value)?,
		})
	}
}
