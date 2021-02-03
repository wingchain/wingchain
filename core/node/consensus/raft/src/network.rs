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

use log::{info, trace};
use tokio::time::Instant;

use crypto::dsa::{Dsa, KeyPair, Verifier};
use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{ConsensusInMessage, ConsensusOutMessage, PeerId};
use primitives::codec::{self, Encode};
use primitives::errors::CommonResult;
use primitives::{Address, Signature};

use crate::protocol::{
	AppendEntriesReq, AppendEntriesRes, RaftMessage, RegisterValidatorReq, RegisterValidatorRes,
	RequestId, RequestIdAware, RequestVoteReq, RequestVoteRes,
};
use crate::state::State;
use crate::stream::{get_address, RaftStream};

/// methods for request/response
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	pub fn register_validator(&mut self, peer_id: PeerId) -> CommonResult<Option<RequestId>> {
		let peer_info = match self.peers.get(&peer_id) {
			Some(v) => v,
			None => return Ok(None),
		};
		trace!("Request validator: peer_id: {}", peer_id);

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
		let request_id = self.request(peer_id.clone(), req)?;

		Ok(Some(request_id))
	}

	pub fn append_entries(
		&mut self,
		address: Address,
		req: AppendEntriesReq,
	) -> CommonResult<Option<RequestId>> {
		let peer_id = match self.known_validators.get(&address) {
			Some(v) => v,
			None => return Ok(None),
		};
		trace!("Append entries: address: {}, req: {:?}", address, req);

		let peer_id = peer_id.clone();
		let request_id = self.request(peer_id, req)?;

		Ok(Some(request_id))
	}

	pub fn request_vote(
		&mut self,
		address: Address,
		req: RequestVoteReq,
	) -> CommonResult<Option<RequestId>> {
		let peer_id = match self.known_validators.get(&address) {
			Some(v) => v,
			None => return Ok(None),
		};
		trace!("Request vote: address: {}, req: {:?}", address, req);

		let peer_id = peer_id.clone();
		let request_id = self.request(peer_id, req)?;

		Ok(Some(request_id))
	}

	fn on_req_register_validator(
		&mut self,
		peer_id: PeerId,
		req: RegisterValidatorReq,
	) -> CommonResult<RegisterValidatorRes> {
		trace!(
			"On req register validator: peer_id: {}, req: {:?}",
			peer_id,
			req
		);

		let mut success = false;
		if let Some(peer_info) = self.peers.get_mut(&peer_id) {
			let dsa = self.support.get_basic()?.dsa.clone();
			let verifier = dsa.verifier_from_public_key(&req.public_key.0)?;
			let message = codec::encode(&peer_info.local_nonce)?;
			let result = verifier.verify(&message, &req.signature.0);
			success = result.is_ok();

			if success {
				let address = get_address(&req.public_key, &self.support)?;
				info!(
					"Register validator accepted: peer_id: {}, address: {}",
					peer_id, address
				);
				peer_info.address = Some(address.clone());
				self.known_validators.insert(address, peer_id);
			}
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
		trace!(
			"On res register validator: peer_id: {}, res: {:?}",
			peer_id,
			res
		);
		Ok(())
	}

	fn on_req_append_entries(
		&mut self,
		address: Address,
		req: AppendEntriesReq,
	) -> CommonResult<AppendEntriesRes> {
		trace!(
			"On req append entries: address: {}, req: {:?}",
			address,
			req
		);
		let (last_log_index, last_log_term) = self.storage.get_last_log_index_term();

		let current_term = self.storage.get_current_term();
		if req.term < current_term {
			return Ok(AppendEntriesRes {
				request_id: req.request_id,
				success: false,
				term: current_term,
				last_log_index,
				last_log_term,
			});
		}

		self.update_next_election_instant(true);
		self.storage.update_commit_log_index(req.commit_log_index)?;

		if req.term != current_term {
			self.storage.update_current_term(req.term)?;
			self.storage.update_current_voted_for(None)?;
		}

		self.update_current_leader(Some(address));

		self.update_state(State::Follower);

		if req.entries.is_empty() {
			return Ok(AppendEntriesRes {
				request_id: req.request_id,
				success: true,
				term: current_term,
				last_log_index,
				last_log_term,
			});
		}

		if req.entries.first().map(|x| x.index) != Some(req.prev_log_index + 1) {
			return Ok(AppendEntriesRes {
				request_id: req.request_id,
				success: false,
				term: current_term,
				last_log_index,
				last_log_term,
			});
		}

		let (last_log_index, last_log_term) = self.storage.get_last_log_index_term();
		let index_and_term_match =
			(req.prev_log_index == last_log_index) && (req.prev_log_term == last_log_term);
		if index_and_term_match {
			self.storage.append_log_entries(req.entries)?;
			let (last_log_index, last_log_term) = self.storage.get_last_log_index_term();
			trace!(
				"Storage appended log entries: last_log_index: {}, last_log_term: {}",
				last_log_index,
				last_log_term
			);

			return Ok(AppendEntriesRes {
				request_id: req.request_id,
				success: true,
				term: current_term,
				last_log_index,
				last_log_term,
			});
		}

		let (base_log_index, base_log_term) = self.storage.get_base_log_index_term();

		let prev_log_exists = self
			.storage
			.get_log_entries(req.prev_log_index..=req.prev_log_index)
			.into_iter()
			.any(|x| x.term == req.prev_log_term)
			|| (req.prev_log_index == base_log_index && req.prev_log_term == base_log_term);

		if !prev_log_exists {
			return Ok(AppendEntriesRes {
				request_id: req.request_id,
				success: false,
				term: current_term,
				last_log_index: base_log_index,
				last_log_term: base_log_term,
			});
		}

		self.storage
			.delete_log_entries((req.prev_log_index + 1)..)?;
		self.storage.append_log_entries(req.entries)?;

		let (last_log_index, last_log_term) = self.storage.get_last_log_index_term();
		trace!(
			"Storage appended log entries: last_log_index: {}, last_log_term: {}",
			last_log_index,
			last_log_term
		);

		Ok(AppendEntriesRes {
			request_id: req.request_id,
			success: true,
			term: current_term,
			last_log_index,
			last_log_term,
		})
	}

	fn on_res_append_entries(
		&mut self,
		address: Address,
		res: AppendEntriesRes,
	) -> CommonResult<()> {
		trace!(
			"On res append entries: address: {}, res: {:?}",
			address,
			res
		);

		if let Some(tx) = &self.append_entries_res_tx {
			let _ = tx.unbounded_send((address, res));
		}
		Ok(())
	}

	fn on_req_request_vote(
		&mut self,
		address: Address,
		req: RequestVoteReq,
	) -> CommonResult<RequestVoteRes> {
		trace!("On req request vote: address: {}, req: {:?}", address, req);

		let current_term = self.storage.get_current_term();
		if req.term < current_term {
			return Ok(RequestVoteRes {
				request_id: req.request_id,
				term: current_term,
				vote_granted: false,
			});
		}

		if let Some(last_heartbeat_instant) = &self.last_heartbeat_instant {
			let now = Instant::now();
			let duration = now.duration_since(*last_heartbeat_instant);
			if self.raft_meta.election_timeout_min >= (duration.as_millis() as u64) {
				return Ok(RequestVoteRes {
					request_id: req.request_id,
					term: current_term,
					vote_granted: false,
				});
			}
		}

		if req.term > current_term {
			self.update_next_election_instant(false);
			self.update_state(State::Follower);

			self.storage.update_current_term(req.term)?;
			self.storage.update_current_voted_for(None)?;
		}

		let (last_log_index, last_log_term) = self.storage.get_last_log_index_term();

		let uptodate = req.last_log_index >= last_log_index && req.last_log_term >= last_log_term;

		if !uptodate {
			return Ok(RequestVoteRes {
				request_id: req.request_id,
				term: current_term,
				vote_granted: false,
			});
		}

		let current_voted_for = self.storage.get_current_voted_for();

		match &current_voted_for {
			Some(voted_address) if voted_address == &address => Ok(RequestVoteRes {
				request_id: req.request_id,
				term: current_term,
				vote_granted: true,
			}),
			Some(_) => Ok(RequestVoteRes {
				request_id: req.request_id,
				term: current_term,
				vote_granted: false,
			}),
			None => {
				self.update_state(State::Follower);
				self.update_next_election_instant(false);

				self.storage.update_current_voted_for(Some(address))?;

				Ok(RequestVoteRes {
					request_id: req.request_id,
					term: current_term,
					vote_granted: true,
				})
			}
		}
	}

	fn on_res_request_vote(&mut self, address: Address, res: RequestVoteRes) -> CommonResult<()> {
		trace!("On res request vote: address: {}, res: {:?}", address, res);

		if let Some(tx) = &self.request_vote_res_tx {
			let _ = tx.unbounded_send((address, res));
		}
		Ok(())
	}
}

/// methods for in messages
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	pub fn on_in_message(&mut self, in_message: ConsensusInMessage) -> CommonResult<()> {
		match in_message {
			ConsensusInMessage::NetworkProtocolOpen {
				peer_id,
				local_nonce,
				remote_nonce,
			} => {
				self.on_network_protocol_open(peer_id, local_nonce, remote_nonce)?;
			}
			ConsensusInMessage::NetworkProtocolClose { peer_id } => {
				self.on_network_protocol_close(peer_id);
			}
			ConsensusInMessage::NetworkMessage { peer_id, message } => {
				self.on_network_message(peer_id, message)?;
			}
			_ => {}
		}
		Ok(())
	}

	fn on_network_protocol_open(
		&mut self,
		peer_id: PeerId,
		local_nonce: u64,
		remote_nonce: u64,
	) -> CommonResult<()> {
		self.peers.insert(
			peer_id.clone(),
			PeerInfo {
				local_nonce,
				remote_nonce,
				address: None,
			},
		);
		self.register_validator(peer_id)?;
		Ok(())
	}

	fn on_network_protocol_close(&mut self, peer_id: PeerId) {
		if let Some(peer_info) = self.peers.get(&peer_id) {
			if let Some(address) = &peer_info.address {
				self.known_validators.remove(address);
			}
		}
		self.peers.remove(&peer_id);
	}

	fn on_network_message(&mut self, peer_id: PeerId, message: Vec<u8>) -> CommonResult<()> {
		let message: RaftMessage = codec::decode(&mut &message[..])?;

		match message {
			RaftMessage::RegisterValidatorReq(req) => {
				let res = self.on_req_register_validator(peer_id.clone(), req)?;
				self.response(peer_id, res)?;
			}
			RaftMessage::RegisterValidatorRes(res) => {
				self.on_res_register_validator(peer_id, res)?;
			}
			RaftMessage::AppendEntriesReq(req) => {
				if let Some(address) = self.get_peer_address(&peer_id) {
					let res = self.on_req_append_entries(address, req)?;
					self.response(peer_id, res)?;
				}
			}
			RaftMessage::AppendEntriesRes(res) => {
				if let Some(address) = self.get_peer_address(&peer_id) {
					self.on_res_append_entries(address, res)?;
				}
			}
			RaftMessage::RequestVoteReq(req) => {
				if let Some(address) = self.get_peer_address(&peer_id) {
					let res = self.on_req_request_vote(address, req)?;
					self.response(peer_id, res)?;
				}
			}
			RaftMessage::RequestVoteRes(res) => {
				if let Some(address) = self.get_peer_address(&peer_id) {
					self.on_res_request_vote(address, res)?;
				}
			}
		};
		Ok(())
	}
}

/// methods for out messages
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	fn request<Req>(&mut self, peer_id: PeerId, mut request: Req) -> CommonResult<RequestId>
	where
		Req: RequestIdAware + Into<RaftMessage>,
	{
		let request_id = Self::next_request_id(&mut self.next_request_id);
		request.set_request_id(request_id.clone());

		let message = request.into();
		let out_message = ConsensusOutMessage::NetworkMessage {
			peer_id,
			message: message.encode(),
		};
		self.send_out_message(out_message)?;

		Ok(request_id)
	}

	fn get_peer_address(&self, peer_id: &PeerId) -> Option<Address> {
		match self.peers.get(peer_id) {
			Some(peer_info) => peer_info.address.clone(),
			None => None,
		}
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

	fn send_out_message(&self, out_message: ConsensusOutMessage) -> CommonResult<()> {
		self.out_tx
			.unbounded_send(out_message)
			.map_err(|e| node_consensus_base::errors::ErrorKind::Channel(Box::new(e)))?;
		Ok(())
	}
}

pub struct PeerInfo {
	local_nonce: u64,
	remote_nonce: u64,
	address: Option<Address>,
}
