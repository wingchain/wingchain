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
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use futures_timer::Delay;

use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{ConsensusOutMessage, PeerId};
use primitives::codec::{self, Encode};
use primitives::errors::CommonResult;

use crate::protocol::{
	RaftMessage, RegisterValidatorReq, RegisterValidatorRes, RequestId, RequestIdAware,
};
use crate::{get_address, RaftStream};
use crypto::dsa::{Dsa, KeyPair, Verifier};
use primitives::Signature;

const REQUEST_TIMEOUT: u64 = 1000;

/// methods for request/response
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	pub fn register_validator(&mut self, peer_id: PeerId) -> CommonResult<()> {
		if let Some(peer_info) = self.peers.get(&peer_id) {
			let message = codec::encode(&peer_info.local_nonce)?;
			let dsa = self.support.get_basic()?.dsa.clone();
			let keypair = dsa.key_pair_from_secret_key(&self.secret_key.0)?;
			let signature = {
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

			let peer_id_clone = peer_id.clone();
			let on_success = move |res: RegisterValidatorRes| {
				info!(
					"Register validator: peer_id: {}, success: {}",
					peer_id_clone, res.success
				);
			};
			let on_failure = || {};

			self.request(
				peer_id,
				req,
				REQUEST_TIMEOUT,
				Arc::new(on_success),
				Arc::new(on_failure),
			)?;
		}

		Ok(())
	}

	pub fn request<Req, Res>(
		&mut self,
		peer_id: PeerId,
		mut request: Req,
		timeout: u64,
		on_success: Arc<dyn Fn(Res) + Send + Sync>,
		on_failure: Arc<dyn Fn() + Send + Sync>,
	) -> CommonResult<()>
	where
		Req: RequestIdAware + Into<RaftMessage>,
		Res: RequestIdAware + TryFrom<RaftMessage> + 'static,
	{
		let request_id = Self::next_request_id(&mut self.next_request_id);

		let on_failure_clone = on_failure.clone();
		let on_success = move |message: RaftMessage| match message.try_into() {
			Ok(v) => on_success(v),
			Err(_) => on_failure_clone(),
		};

		self.requests
			.insert(request_id.clone(), (Arc::new(on_success), on_failure));

		let timer_result = request_id.clone();
		self.requests_timer.push(
			async move {
				Delay::new(Duration::from_millis(timeout)).await;
				timer_result
			}
			.boxed(),
		);

		request.set_request_id(request_id);
		let message = request.into();

		let out_message = ConsensusOutMessage::NetworkMessage {
			peer_id,
			message: message.encode(),
		};
		self.send_out_message(out_message)?;

		Ok(())
	}

	pub fn on_network_message(&mut self, peer_id: PeerId, message: Vec<u8>) -> CommonResult<()> {
		let message: RaftMessage = codec::decode(&mut &message[..])?;

		match message {
			RaftMessage::RegisterValidatorReq(req) => {
				let res = self.on_register_validator(peer_id.clone(), req)?;
				self.response(peer_id, res)?;
			}
			RaftMessage::RegisterValidatorRes(res) => {
				self.callback(res);
			}
		};
		Ok(())
	}

	pub fn on_requests_timer_trigger(&mut self, request_id: RequestId) -> CommonResult<()> {
		if let Some((_, on_failure)) = self.requests.remove(&request_id) {
			on_failure();
		}
		Ok(())
	}

	fn on_register_validator(
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
				"Accept registering validator: peer_id: {}, address: {}",
				peer_id, address
			);
			self.known_validators.insert(address, peer_id);
		}

		Ok(RegisterValidatorRes {
			request_id: req.request_id,
			success,
		})
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

	fn callback<Res>(&mut self, res: Res)
	where
		Res: RequestIdAware + Into<RaftMessage>,
	{
		let request_id = res.get_request_id();
		if let Some((on_success, _)) = self.requests.remove(&request_id) {
			let message = res.into();
			on_success(message);
		}
	}

	fn next_request_id(request_id: &mut RequestId) -> RequestId {
		let new = RequestId(request_id.0.checked_add(1).unwrap_or(0));
		std::mem::replace(request_id, new)
	}
}
