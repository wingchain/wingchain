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

use log::info;

use crypto::dsa::{Dsa, KeyPair, Verifier};
use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{ConsensusOutMessage, PeerId};
use primitives::codec::{self, Encode};
use primitives::errors::CommonResult;
use primitives::Signature;

use crate::protocol::{
	RaftMessage, RegisterValidatorReq, RegisterValidatorRes, RequestId, RequestIdAware,
};
use crate::{get_address, RaftStream};

/// methods for request/response
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	pub fn register_validator(&mut self, peer_id: PeerId) -> CommonResult<()> {
		if let Some(peer_info) = self.peers.get(&peer_id) {
			let message = codec::encode(&peer_info.remote_nonce)?;
			let dsa = self.support.get_basic()?.dsa.clone();
			let signature = {
				let keypair = dsa.key_pair_from_secret_key(&self.secret_key.0)?;
				let (_, _, signature_len) = dsa.length().into();
				let mut out = vec![0u8; signature_len];
				keypair.sign(&message, &mut out);
				Signature(out)
			};

			let req = RegisterValidatorReq {
				request_id: RequestId(0),
				public_key: self.public_key.clone(),
				signature,
			};

			self.request(peer_id.clone(), req)?;
		}

		Ok(())
	}

	pub fn on_network_message(&mut self, peer_id: PeerId, message: Vec<u8>) -> CommonResult<()> {
		let message: RaftMessage = codec::decode(&mut &message[..])?;

		match message {
			RaftMessage::RegisterValidatorReq(req) => {
				let res = self.on_req_register_validator(peer_id.clone(), req)?;
				self.response(peer_id, res)?;
			}
			RaftMessage::RegisterValidatorRes(res) => {
				self.on_res_register_validator(peer_id, res)?;
			}
		};
		Ok(())
	}

	fn on_req_register_validator(
		&mut self,
		peer_id: PeerId,
		req: RegisterValidatorReq,
	) -> CommonResult<RegisterValidatorRes> {
		let mut success = false;
		if let Some(peer_info) = self.peers.get(&peer_id) {
			let dsa = self.support.get_basic()?.dsa.clone();
			let verifier = dsa.verifier_from_public_key(&req.public_key.0)?;
			let message = codec::encode(&peer_info.local_nonce)?;
			let result = verifier.verify(&message, &req.signature.0);
			success = result.is_ok();
		}

		if success {
			let address = get_address(&req.public_key, &self.support)?;
			info!(
				"Register validator accepted: peer_id: {}, address: {}",
				peer_id, address
			);
			self.known_validators.insert(address, peer_id);
		}

		Ok(RegisterValidatorRes {
			request_id: req.request_id,
			success,
		})
	}

	fn on_res_register_validator(
		&mut self,
		peer_id: PeerId,
		res: RegisterValidatorRes,
	) -> CommonResult<()> {
		info!(
			"Register validator result: peer_id: {}, success: {}",
			peer_id, res.success
		);
		Ok(())
	}

	fn request<Req>(&mut self, peer_id: PeerId, mut request: Req) -> CommonResult<()>
	where
		Req: RequestIdAware + Into<RaftMessage>,
	{
		let request_id = Self::next_request_id(&mut self.next_request_id);
		request.set_request_id(request_id);

		let message = request.into();
		let out_message = ConsensusOutMessage::NetworkMessage {
			peer_id,
			message: message.encode(),
		};
		self.send_out_message(out_message)?;

		Ok(())
	}

	fn response<Res>(&self, peer_id: PeerId, res: Res) -> CommonResult<()>
	where
		Res: Into<RaftMessage>,
	{
		let message = res.into();
		let out_message = ConsensusOutMessage::NetworkMessage {
			peer_id,
			message: message.encode(),
		};
		self.send_out_message(out_message)?;
		Ok(())
	}

	fn next_request_id(request_id: &mut RequestId) -> RequestId {
		let new = RequestId(request_id.0.checked_add(1).unwrap_or(0));
		std::mem::replace(request_id, new)
	}
}
