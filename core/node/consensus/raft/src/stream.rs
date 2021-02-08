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

use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use log::{info, trace};
use rand::{thread_rng, Rng};
use tokio::time::Duration;
use tokio::time::Instant;

use crypto::address::Address as AddressT;
use crypto::dsa::{Dsa, KeyPair, Verifier as VerifierT};
use node_chain::ChainCommitBlockParams;
use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{ConsensusInMessage, ConsensusOutMessage, PeerId};
use node_executor::module::raft::{Authorities, Meta};
use primitives::codec;
use primitives::codec::Encode;
use primitives::errors::{Catchable, CommonResult};
use primitives::{
	Address, BlockNumber, BuildBlockParams, FullTransaction, Hash, PublicKey, SecretKey, Signature,
	Transaction,
};

use crate::config::{
	DEFAULT_EXTRA_ELECTION_TIMEOUT_PER_KB, DEFAULT_INIT_EXTRA_ELECTION_TIMEOUT,
	DEFAULT_REQUEST_PROPOSAL_MIN_INTERVAL,
};
use crate::errors::ErrorKind;
use crate::proof::Proof;
use crate::protocol::{
	AppendEntriesReq, AppendEntriesRes, EntryData, Proposal, RaftMessage, RegisterValidatorReq,
	RegisterValidatorRes, RequestId, RequestIdAware, RequestProposalReq, RequestProposalRes,
	RequestVoteReq, RequestVoteRes,
};
use crate::storage::Storage;
use crate::stream::state::{CandidateState, FollowerState, LeaderState, ObserverState, State};
use crate::verifier::VerifyError;
use crate::{get_raft_authorities, RaftConfig};
use node_consensus_base::errors::map_channel_err;
use node_consensus_primitives::CONSENSUS_RAFT;
use serde::Serialize;
use serde_json::Value;
use std::convert::TryInto;

mod state;

#[allow(clippy::type_complexity)]
pub struct RaftStream<S>
where
	S: ConsensusSupport,
{
	/// External support
	support: Arc<S>,

	/// Raft meta data
	raft_meta: Arc<Meta>,

	/// Raft config
	raft_config: Arc<RaftConfig>,

	/// Secret key of current validator
	secret_key: SecretKey,

	/// Public key of current validator
	public_key: PublicKey,

	/// Address of current validator
	address: Address,

	/// Out message sender
	out_tx: UnboundedSender<ConsensusOutMessage>,

	/// In message receiver
	in_rx: UnboundedReceiver<ConsensusInMessage>,

	/// All connected peers
	peers: HashMap<PeerId, PeerInfo>,

	/// Known validators registered to us
	known_validators: HashMap<Address, PeerId>,

	/// Next request id
	next_request_id: RequestId,

	/// Persistant storage
	storage: Arc<Storage<S>>,

	/// Raft current leader
	current_leader: Option<Address>,

	/// Next election instant
	next_election_instant: Option<Instant>,

	/// Last heartbeat instant
	last_heartbeat_instant: Option<Instant>,

	/// Raft state
	state: State,

	/// Authorities
	authorities: Authorities,

	/// Internal message sender
	internal_tx: UnboundedSender<InternalMessage>,

	/// Internal message receiver
	internal_rx: UnboundedReceiver<InternalMessage>,

	/// Store last request proposal instant
	/// to avoid request too frequently
	last_request_proposal_instant: Option<Instant>,

	/// Received proposal that need wait
	pending_proposal: Option<Proposal>,

	/// Commit block params
	/// When building or verifying a proposal, the commit_block_params
	/// is returned. We just keep it for later use
	commit_block_params: Option<ChainCommitBlockParams>,
}

impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	pub fn spawn(
		support: Arc<S>,
		raft_meta: Meta,
		raft_config: RaftConfig,
		out_tx: UnboundedSender<ConsensusOutMessage>,
		in_rx: UnboundedReceiver<ConsensusInMessage>,
	) -> CommonResult<()> {
		let secret_key = match &raft_config.secret_key {
			Some(v) => v.clone(),
			None => return Ok(()),
		};

		let public_key = get_public_key(&secret_key, &support)?;
		let address = get_address(&public_key, &support)?;
		let raft_meta = Arc::new(raft_meta);
		let raft_config = Arc::new(raft_config);
		let storage = Arc::new(Storage::new(support.clone())?);
		let confirmed_number = support.get_current_state().confirmed_number;
		let authorities = get_raft_authorities(&support, &confirmed_number)?;

		let (internal_tx, internal_rx) = unbounded();

		let this = Self {
			support,
			raft_meta,
			raft_config,
			secret_key,
			public_key,
			address,
			out_tx,
			in_rx,
			peers: HashMap::new(),
			known_validators: HashMap::new(),
			next_request_id: RequestId(0),
			storage,
			current_leader: None,
			next_election_instant: None,
			last_heartbeat_instant: None,
			state: State::Observer,
			authorities,
			internal_tx,
			internal_rx,
			last_request_proposal_instant: None,
			pending_proposal: None,
			commit_block_params: None,
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) -> CommonResult<()> {
		info!("Start raft work");

		self.update_state(State::Follower);
		if self.state == State::Follower {
			let init_extra_election_timeout = self
				.raft_config
				.init_extra_election_timeout
				.unwrap_or(DEFAULT_INIT_EXTRA_ELECTION_TIMEOUT);
			let next_election_instant = Instant::now()
				+ Duration::from_millis(self.rand_election_timeout() + init_extra_election_timeout);
			self.next_election_instant = Some(next_election_instant);
		}

		loop {
			info!("Work as {:?}", self.state);
			match self.state {
				State::Leader => LeaderState::new(&mut self).start().await?,
				State::Candidate => CandidateState::new(&mut self).start().await?,
				State::Follower => FollowerState::new(&mut self).start().await?,
				State::Observer => ObserverState::new(&mut self).start().await?,
			}
		}
	}

	fn next_election_instant(&mut self) -> Instant {
		match self.next_election_instant {
			Some(inst) => inst,
			None => {
				let inst = Instant::now() + Duration::from_millis(self.rand_election_timeout());
				self.next_election_instant = Some(inst);
				inst
			}
		}
	}

	fn update_next_election_instant(&mut self, extra: u64, heartbeat: bool) {
		let now = Instant::now();
		self.next_election_instant = Some(
			now + Duration::from_millis(self.rand_election_timeout())
				+ Duration::from_millis(extra),
		);
		if heartbeat {
			self.last_heartbeat_instant = Some(now);
		}
	}

	fn update_current_leader(&mut self, leader: Option<Address>) {
		self.current_leader = leader;
	}

	fn update_state(&mut self, state: State) {
		if state == State::Follower && !self.authorities.members.contains(&self.address) {
			self.state = State::Observer;
			return;
		}
		self.state = state;
	}

	fn rand_election_timeout(&self) -> u64 {
		thread_rng().gen_range(
			self.raft_meta.election_timeout_min,
			self.raft_meta.election_timeout_max,
		)
	}

	fn consensus_state(&self) -> CommonResult<ConsensusState> {
		let current_state = self.support.get_current_state();
		let authorities = get_raft_authorities(&self.support, &current_state.confirmed_number)?;

		Ok(ConsensusState {
			consensus_name: CONSENSUS_RAFT.to_string(),
			address: self.address.clone(),
			meta: (*self.raft_meta).clone(),
			authorities,
			current_leader: self.current_leader.clone(),
		})
	}
}

impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	fn register_validator(&mut self, peer_id: PeerId) -> CommonResult<Option<RequestId>> {
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

	fn append_entries(
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

	fn request_vote(
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

	fn request_proposal(
		&mut self,
		address: Address,
		req: RequestProposalReq,
	) -> CommonResult<Option<RequestId>> {
		let peer_id = match self.known_validators.get(&address) {
			Some(v) => v,
			None => return Ok(None),
		};
		trace!("Request proposal: address: {}, req: {:?}", address, req);

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

		self.update_next_election_instant(0, true);
		self.storage.update_commit_log_index(req.commit_log_index)?;

		if req.term != current_term {
			self.storage.update_current_term(req.term)?;
			self.storage.update_current_voted_for(None)?;
		}

		self.update_current_leader(Some(address.clone()));

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
			let result = self.perform_append_entries(address, req);
			return result;
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
		self.perform_append_entries(address, req)
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

		self.internal_tx
			.unbounded_send(InternalMessage::AppendEntriesRes { address, res })
			.map_err(map_channel_err)?;

		Ok(())
	}

	fn perform_append_entries(
		&mut self,
		address: Address,
		req: AppendEntriesReq,
	) -> CommonResult<AppendEntriesRes> {
		let remote_proposal_block_hash = req.entries.iter().find_map(|x| match &x.data {
			EntryData::Proposal { block_hash } => Some(block_hash),
			_ => None,
		});

		if let Some(remote_proposal_block_hash) = remote_proposal_block_hash {
			// clear discordant local proposal
			let proposal_accordant = self.storage.get_proposal_using(|x| {
				x.as_ref()
					.map(|p| &p.block_hash == remote_proposal_block_hash)
			});
			if proposal_accordant != Some(true) {
				self.storage.update_proposal(None)?;
			}

			// clear discordant pending proposal
			let pending_proposal_accordant = self
				.pending_proposal
				.as_ref()
				.map(|x| &x.block_hash == remote_proposal_block_hash);
			if pending_proposal_accordant != Some(true) {
				self.pending_proposal = None;
			}
			// verify pending proposal
			if let Some(proposal) = self.pending_proposal.take() {
				let verifier = crate::verifier::Verifier::new(self.support.clone())?;
				let mut proposal = Some(proposal);
				let result = verifier.verify_proposal(&mut proposal);
				let result_desc = match &result {
					Ok(_v) => "Ok".to_string(),
					Err(e) => e.to_string(),
				};
				trace!("Proposal verify result: {}", result_desc);
				let action = self.on_proposal_verify_result(result)?;
				match action {
					VerifyAction::Ok(proposal, commit_block_params) => {
						self.commit_block_params = Some(commit_block_params);
						self.storage.update_proposal(Some(proposal))?;
					}
					VerifyAction::Wait => {
						// proposal has not been taken
						self.pending_proposal = proposal;
					}
					VerifyAction::Discard => {}
				}
			}

			let local_proposal_exist = self.storage.get_proposal_using(|x| x.is_some());
			if !local_proposal_exist {
				// request proposal if needed
				let mut request_proposal_now = true;

				// requested recently
				if let Some(last_request_proposal_instant) = &self.last_request_proposal_instant {
					let now = Instant::now();
					let duration = now.duration_since(*last_request_proposal_instant);
					let request_proposal_min_interval = self
						.raft_config
						.request_proposal_min_interval
						.unwrap_or(DEFAULT_REQUEST_PROPOSAL_MIN_INTERVAL);
					if request_proposal_min_interval >= (duration.as_millis() as u64) {
						request_proposal_now = false;
					}
				}

				// pending proposal exists
				if self.pending_proposal.is_some() {
					request_proposal_now = false;
				}

				if request_proposal_now {
					self.request_proposal(
						address,
						RequestProposalReq {
							request_id: RequestId(0),
						},
					)?;
					self.last_request_proposal_instant = Some(Instant::now());
				}

				let (last_log_index, last_log_term) = self.storage.get_last_log_index_term();
				return Ok(AppendEntriesRes {
					request_id: req.request_id,
					success: false,
					term: self.storage.get_current_term(),
					last_log_index,
					last_log_term,
				});
			}
		}

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
			term: self.storage.get_current_term(),
			last_log_index,
			last_log_term,
		})
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
			self.update_next_election_instant(0, false);
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
				self.update_next_election_instant(0, false);

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

		self.internal_tx
			.unbounded_send(InternalMessage::RequestVoteRes { address, res })
			.map_err(map_channel_err)?;

		Ok(())
	}

	fn on_req_request_proposal(
		&mut self,
		address: Address,
		req: RequestProposalReq,
	) -> CommonResult<RequestProposalRes> {
		trace!(
			"On req request proposal: address: {}, req: {:?}",
			address,
			req
		);

		let proposal = self.storage.get_proposal().ok_or_else(|| {
			node_consensus_base::errors::ErrorKind::Data("Missing proposal".to_string())
		})?;

		let proposal_size = codec::encode(&proposal)?.len() as u64;
		let extra_election_timeout_per_kb = self
			.raft_config
			.extra_election_timeout_per_kb
			.unwrap_or(DEFAULT_EXTRA_ELECTION_TIMEOUT_PER_KB);
		let extra_election_timeout = extra_election_timeout_per_kb * proposal_size / 1024;

		// send extra election timeout
		if let Some(peer_id) = self.known_validators.get(&address) {
			self.response(
				peer_id.clone(),
				RequestProposalRes {
					request_id: RequestId(0),
					extra_election_timeout,
					proposal: None,
				},
			)?;
		}

		// send proposal
		Ok(RequestProposalRes {
			request_id: req.request_id,
			extra_election_timeout: 0,
			proposal: Some(proposal),
		})
	}

	fn on_res_request_proposal(
		&mut self,
		address: Address,
		res: RequestProposalRes,
	) -> CommonResult<()> {
		trace!(
			"On res request proposal: address: {}, res: {:?}",
			address,
			res
		);
		self.update_next_election_instant(res.extra_election_timeout, false);

		if let Some(proposal) = res.proposal {
			let result = self.on_proposal(proposal);
			self.update_next_election_instant(0, false);
			result?;
		}

		Ok(())
	}

	fn on_proposal(&mut self, proposal: Proposal) -> CommonResult<()> {
		let verifier = crate::verifier::Verifier::new(self.support.clone())?;
		let mut proposal = Some(proposal);
		let result = verifier.verify_proposal(&mut proposal);
		let result_desc = match &result {
			Ok(_v) => "Ok".to_string(),
			Err(e) => e.to_string(),
		};
		trace!("Proposal verify result: {}", result_desc);
		let action = self.on_proposal_verify_result(result)?;
		match action {
			VerifyAction::Ok(proposal, commit_block_params) => {
				self.commit_block_params = Some(commit_block_params);
				self.storage.update_proposal(Some(proposal))?;
			}
			VerifyAction::Wait => {
				// proposal has not been taken
				self.pending_proposal = proposal;
			}
			VerifyAction::Discard => {}
		}
		Ok(())
	}

	fn on_proposal_verify_result(
		&self,
		result: CommonResult<(Proposal, ChainCommitBlockParams)>,
	) -> CommonResult<VerifyAction> {
		let action = result
			.and_then(|(proposal, commit_block_params)| {
				self.on_proposal_verify_ok(proposal, commit_block_params)
			})
			.or_else_catch::<ErrorKind, _>(|e| match e {
				ErrorKind::VerifyError(e) => Some(self.on_proposal_verify_err(e)),
			})?;
		Ok(action)
	}

	fn on_proposal_verify_ok(
		&self,
		proposal: Proposal,
		commit_block_params: ChainCommitBlockParams,
	) -> CommonResult<VerifyAction> {
		let action = VerifyAction::Ok(proposal, commit_block_params);
		Ok(action)
	}

	fn on_proposal_verify_err(&self, e: &VerifyError) -> CommonResult<VerifyAction> {
		let action = match e {
			VerifyError::ShouldWait => VerifyAction::Wait,
			VerifyError::Duplicated => VerifyAction::Discard,
			VerifyError::NotBest => VerifyAction::Discard,
			VerifyError::InvalidExecutionGap => VerifyAction::Discard,
			VerifyError::InvalidHeader(_) => VerifyAction::Discard,
			VerifyError::DuplicatedTx(_) => VerifyAction::Discard,
			VerifyError::InvalidTx(_) => VerifyAction::Discard,
		};
		Ok(action)
	}
}

/// methods for in messages
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	fn on_in_message(&mut self, in_message: ConsensusInMessage) -> CommonResult<()> {
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
			ConsensusInMessage::BlockCommitted { number, block_hash } => {
				self.on_block_committed(number, block_hash)?;
			}
			ConsensusInMessage::Generate => {
				self.internal_tx
					.unbounded_send(InternalMessage::Generate)
					.map_err(map_channel_err)?;
			}
			ConsensusInMessage::GetConsensusState { tx } => {
				let value = serde_json::to_value(self.consensus_state()?).unwrap_or(Value::Null);
				let _ = tx.send(value);
			}
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
					self.internal_tx
						.unbounded_send(InternalMessage::LogUpdated)
						.map_err(map_channel_err)?;
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
			RaftMessage::RequestProposalReq(req) => {
				if let Some(address) = self.get_peer_address(&peer_id) {
					let res = self.on_req_request_proposal(address, req)?;
					self.response(peer_id, res)?;
				}
			}
			RaftMessage::RequestProposalRes(res) => {
				if let Some(address) = self.get_peer_address(&peer_id) {
					self.on_res_request_proposal(address, res)?;
				}
			}
		};
		Ok(())
	}
}

/// methods for internal messages
#[allow(clippy::single_match)]
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	fn on_internal_message(&mut self, internal_message: InternalMessage) -> CommonResult<()> {
		match internal_message {
			InternalMessage::LogUpdated => {
				self.on_log_updated()?;
			}
			_ => {}
		}
		Ok(())
	}

	fn on_log_updated(&mut self) -> CommonResult<()> {
		let commit_log_index = self.storage.get_commit_log_index();
		let committed_proposal_block_hash = self
			.storage
			.get_log_entries(..=commit_log_index)
			.iter()
			.find_map(|x| match &x.data {
				EntryData::Proposal { block_hash } => Some(block_hash.clone()),
				_ => None,
			});

		if let Some(committed_proposal_block_hash) = committed_proposal_block_hash {
			let proposal = self.storage.get_proposal().ok_or_else(|| {
				node_consensus_base::errors::ErrorKind::Data("Missing proposal".to_string())
			})?;

			// clear discordant commit_block_params
			let commit_block_params_accordant = self
				.commit_block_params
				.as_ref()
				.map(|x| x.block_hash == committed_proposal_block_hash);
			if commit_block_params_accordant != Some(true) {
				self.commit_block_params = None;
			}

			// take kept commit_block_params or build one
			let mut commit_block_params = match self.commit_block_params.take() {
				Some(v) => v,
				None => {
					let support = self.support.clone();
					let convert_txs =
						|txs: Vec<Transaction>| -> CommonResult<Vec<Arc<FullTransaction>>> {
							txs.into_iter()
								.map(|tx| -> CommonResult<Arc<FullTransaction>> {
									let tx_hash = support.hash_transaction(&tx)?;
									Ok(Arc::new(FullTransaction { tx_hash, tx }))
								})
								.collect()
						};

					let build_block_params = BuildBlockParams {
						number: proposal.number,
						timestamp: proposal.timestamp,
						meta_txs: convert_txs(proposal.meta_txs)?,
						payload_txs: convert_txs(proposal.payload_txs)?,
						execution_number: proposal.execution_number,
					};
					self.support.build_block(build_block_params)?
				}
			};

			let (log_index, log_term) = self.storage.get_last_log_index_term();
			let proof = Proof::new(
				&commit_block_params.block_hash,
				log_index,
				log_term,
				&self.secret_key,
				self.support.get_basic()?.dsa.clone(),
			)?;
			commit_block_params.proof = proof.try_into()?;

			let number = commit_block_params.header.number;
			let block_hash = commit_block_params.block_hash.clone();

			self.support.commit_block(commit_block_params)?;

			info!(
				"Block committed: number: {}, block_hash: {}",
				number, block_hash
			);
		}

		Ok(())
	}

	fn on_block_committed(&self, number: BlockNumber, block_hash: Hash) -> CommonResult<()> {
		self.storage.refresh()?;
		info!(
			"Storage refreshed: number: {}, block_hash: {}",
			number, block_hash
		);

		// TODO update authorities

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
			.map_err(map_channel_err)?;
		Ok(())
	}
}

fn get_public_key<S: ConsensusSupport>(
	secret_key: &SecretKey,
	support: &Arc<S>,
) -> CommonResult<PublicKey> {
	let dsa = support.get_basic()?.dsa.clone();
	let (_, public_key_len, _) = dsa.length().into();
	let mut public_key = vec![0u8; public_key_len];
	dsa.key_pair_from_secret_key(&secret_key.0)?
		.public_key(&mut public_key);

	let public_key = PublicKey(public_key);
	Ok(public_key)
}

fn get_address<S: ConsensusSupport>(
	public_key: &PublicKey,
	support: &Arc<S>,
) -> CommonResult<Address> {
	let addresser = support.get_basic()?.address.clone();
	let address_len = addresser.length().into();
	let mut address = vec![0u8; address_len];
	addresser.address(&mut address, &public_key.0);

	let address = Address(address);
	Ok(address)
}

struct PeerInfo {
	local_nonce: u64,
	remote_nonce: u64,
	address: Option<Address>,
}

#[derive(Debug)]
enum VerifyAction {
	Ok(Proposal, ChainCommitBlockParams),
	Wait,
	Discard,
}

enum InternalMessage {
	LogUpdated,
	/// The candidate state will follow RequestVoteRes message
	RequestVoteRes {
		address: Address,
		res: RequestVoteRes,
	},
	/// The leader state will follow AppendEntriesRes message
	AppendEntriesRes {
		address: Address,
		res: AppendEntriesRes,
	},
	/// The leader state will follow Generate message
	Generate,
}

#[derive(Serialize)]
struct ConsensusState {
	consensus_name: String,
	address: Address,
	meta: Meta,
	authorities: Authorities,
	current_leader: Option<Address>,
}
