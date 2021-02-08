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
use futures::prelude::*;
use log::{debug, error, info, trace};
use tokio::time::{interval, sleep_until, Duration, Interval};

use node_consensus_base::support::ConsensusSupport;
use node_executor::module::raft::Meta;
use primitives::errors::CommonResult;
use primitives::{Address, Hash};

use crate::protocol::{
	AppendEntriesReq, AppendEntriesRes, Entry, EntryData, Proposal, RequestId, RequestVoteReq,
	RequestVoteRes,
};
use crate::storage::Storage;
use crate::stream::{InternalMessage, RaftStream};
use node_consensus_base::errors::map_channel_err;
use node_consensus_base::scheduler::{ScheduleInfo, Scheduler};
use std::time::SystemTime;

#[derive(PartialEq, Debug)]
pub enum State {
	Leader,
	Candidate,
	Follower,
	Observer,
}

pub struct LeaderState<'a, S>
where
	S: ConsensusSupport,
{
	stream: &'a mut RaftStream<S>,
	replications: HashMap<Address, Replication>,
	replication_out_tx: UnboundedSender<ReplicationOutMessage>,
	replication_out_rx: UnboundedReceiver<ReplicationOutMessage>,
}

impl<'a, S> LeaderState<'a, S>
where
	S: ConsensusSupport,
{
	pub fn new(stream: &'a mut RaftStream<S>) -> Self {
		let (replication_out_tx, replication_out_rx) = unbounded();
		Self {
			stream,
			replications: HashMap::new(),
			replication_out_tx,
			replication_out_rx,
		}
	}
	pub async fn start(mut self) -> CommonResult<()> {
		let addresses = &self.stream.authorities.members;
		let (base_log_index, base_log_term) = self.stream.storage.get_base_log_index_term();
		for target in addresses {
			if target != &self.stream.address {
				let replication = Replication::new(
					self.stream.raft_meta.clone(),
					self.replication_out_tx.clone(),
					self.stream.storage.clone(),
					target.clone(),
					base_log_index,
					base_log_term,
				)?;
				self.replications.insert(target.clone(), replication);
			}
		}

		self.stream.last_heartbeat_instant = None;
		self.stream.next_election_instant = None;
		self.stream
			.update_current_leader(Some(self.stream.address.clone()));

		self.append_init_entry()?;

		let mut scheduler = Scheduler::new(self.stream.raft_meta.block_interval);

		loop {
			if self.stream.state != State::Leader {
				return Ok(());
			}
			tokio::select! {
				Some(schedule_info) = scheduler.next() => {
					self.work(schedule_info)
					.unwrap_or_else(|e| error!("Raft stream handle work error: {}", e));
				}
				Some(internal_message) = self.stream.internal_rx.next() => {
					match internal_message{
						InternalMessage::AppendEntriesRes {address, res} => {
							self.on_append_entries_res(address, res)
								.unwrap_or_else(|e| error!("Raft stream handle append entries res error: {}", e));
						},
						InternalMessage::Generate => {
							self.generate()
								.unwrap_or_else(|e| error!("Raft stream handle generate message error: {}", e));
						},
						_ => {
							self.stream.on_internal_message(internal_message)
								.unwrap_or_else(|e| error!("Raft stream handle internal message error: {}", e));
						}
					}
				},
				Some(in_message) = self.stream.in_rx.next() => {
					self.stream.on_in_message(in_message)
						.unwrap_or_else(|e| error!("Raft stream handle in message error: {}", e));
				},
				Some(replication_out_message) = self.replication_out_rx.next() => {
					self.on_replication_out_message(replication_out_message)
						.unwrap_or_else(|e| error!("Raft stream handle replication out message error: {}", e));
				}
			}
		}
	}

	fn generate(&mut self) -> CommonResult<()> {
		let timestamp = SystemTime::now();
		let timestamp = timestamp
			.duration_since(SystemTime::UNIX_EPOCH)
			.map_err(|_| node_consensus_base::errors::ErrorKind::Time)?;
		let timestamp = timestamp.as_millis() as u64;
		let schedule_info = ScheduleInfo { timestamp };
		self.work(schedule_info)?;
		Ok(())
	}

	fn work(&mut self, schedule_info: ScheduleInfo) -> CommonResult<()> {
		let contains_proposal = self
			.stream
			.storage
			.get_log_entries(..)
			.iter()
			.any(|x| matches!(x.data, EntryData::Proposal {..}));
		if contains_proposal {
			return Ok(());
		}

		let build_block_params = self.stream.support.prepare_block(schedule_info)?;

		let mut proposal = Proposal {
			block_hash: Hash(vec![]),
			number: build_block_params.number,
			timestamp: build_block_params.timestamp,
			meta_txs: build_block_params
				.meta_txs
				.iter()
				.map(|x| x.tx.clone())
				.collect(),
			payload_txs: build_block_params
				.payload_txs
				.iter()
				.map(|x| x.tx.clone())
				.collect(),
			execution_number: build_block_params.execution_number,
		};

		let commit_block_params = self.stream.support.build_block(build_block_params)?;
		proposal.block_hash = commit_block_params.block_hash.clone();

		let number = proposal.number;
		let execution_number = proposal.execution_number;
		let block_hash = commit_block_params.block_hash.clone();

		self.stream.commit_block_params = Some(commit_block_params);

		trace!(
			"Proposal: number: {}, execution_number: {}, block_hash: {}",
			number,
			execution_number,
			block_hash
		);

		self.stream.storage.update_proposal(Some(proposal))?;

		// append proposal entry
		let (last_log_index, _) = self.stream.storage.get_last_log_index_term();
		let entry = Entry {
			term: self.stream.storage.get_current_term(),
			index: last_log_index + 1,
			data: EntryData::Proposal { block_hash },
		};
		self.stream.storage.append_log_entries(vec![entry])?;

		Ok(())
	}

	fn on_replication_out_message(
		&mut self,
		replication_out_message: ReplicationOutMessage,
	) -> CommonResult<()> {
		match replication_out_message {
			ReplicationOutMessage::AppendEntriesReq { address, req } => {
				if let Some(replication) = self.replications.get(&address) {
					let request_id = self.stream.append_entries(address, req)?;
					let in_message = ReplicationInMessage::AppendEntriesReqResult { request_id };
					replication
						.in_tx
						.unbounded_send(in_message)
						.map_err(map_channel_err)?;
				}
			}
			ReplicationOutMessage::UpdateMatchIndex {
				address,
				match_index,
			} => {
				self.on_update_match_index(address, match_index)?;
			}
			ReplicationOutMessage::UpdateState { state } => {
				self.stream.update_state(state);
			}
		}
		Ok(())
	}

	fn on_update_match_index(&mut self, address: Address, match_index: u64) -> CommonResult<()> {
		if let Some(replication) = self.replications.get_mut(&address) {
			replication.match_index = match_index;

			let match_indices = self
				.replications
				.iter()
				.map(|(_, v)| v.match_index)
				.collect::<Vec<_>>();
			let old_commit_log_index = self.stream.storage.get_commit_log_index();
			let new_commit_log_index =
				get_new_commit_log_index(match_indices, old_commit_log_index);
			if new_commit_log_index != old_commit_log_index {
				self.stream
					.storage
					.update_commit_log_index(new_commit_log_index)?;
				trace!("Storage updated commit_log_index: {}", new_commit_log_index);
				self.stream
					.internal_tx
					.unbounded_send(InternalMessage::LogUpdated)
					.map_err(map_channel_err)?;
			}
		}
		Ok(())
	}

	fn on_append_entries_res(
		&mut self,
		address: Address,
		res: AppendEntriesRes,
	) -> CommonResult<()> {
		if let Some(replication) = self.replications.get(&address) {
			let in_message = ReplicationInMessage::AppendEntriesRes { res };
			replication
				.in_tx
				.unbounded_send(in_message)
				.map_err(map_channel_err)?;
		}
		Ok(())
	}

	fn append_init_entry(&self) -> CommonResult<()> {
		let (last_log_index, _) = self.stream.storage.get_last_log_index_term();
		let entry = Entry {
			term: self.stream.storage.get_current_term(),
			index: last_log_index + 1,
			data: EntryData::Blank,
		};
		self.stream.storage.append_log_entries(vec![entry])?;
		Ok(())
	}
}

fn get_new_commit_log_index(mut match_indices: Vec<u64>, old_commit_log_index: u64) -> u64 {
	// make match_indices len equal to authorities len
	match_indices.push(old_commit_log_index);
	// reverse sort
	match_indices.sort_unstable_by(|a, b| b.cmp(a));

	let offset = match_indices.len() / 2;

	let new_commit_log_index = match_indices[offset];
	u64::max(new_commit_log_index, old_commit_log_index)
}

pub struct CandidateState<'a, S>
where
	S: ConsensusSupport,
{
	stream: &'a mut RaftStream<S>,
	votes_granted: u64,
	votes_needed: u64,
	requests: HashMap<RequestId, ()>,
}

impl<'a, S> CandidateState<'a, S>
where
	S: ConsensusSupport,
{
	pub fn new(stream: &'a mut RaftStream<S>) -> Self {
		Self {
			stream,
			votes_granted: 0,
			votes_needed: 0,
			requests: Default::default(),
		}
	}
	pub async fn start(mut self) -> CommonResult<()> {
		loop {
			if self.stream.state != State::Candidate {
				return Ok(());
			}
			self.votes_granted = 1;
			let authorities_len = self.stream.authorities.members.len();
			self.votes_needed = ((authorities_len / 2) + 1) as u64;
			self.requests.clear();

			self.stream.update_next_election_instant(0, false);
			self.stream.update_current_leader(None);

			self.stream
				.storage
				.update_current_term(self.stream.storage.get_current_term() + 1)?;
			self.stream
				.storage
				.update_current_voted_for(Some(self.stream.address.clone()))?;

			self.request_vote()?;

			loop {
				if self.stream.state != State::Candidate {
					return Ok(());
				}

				let next_election = sleep_until(self.stream.next_election_instant());
				tokio::select! {
					_ = next_election => {
						break;
					},
					Some(internal_message) = self.stream.internal_rx.next() => {
						match internal_message {
							InternalMessage::RequestVoteRes {address, res} => {
								self.on_res_request_vote(address, res)
									.unwrap_or_else(|e| error!("Raft stream handle request vote res error: {}", e));
							},
							_ => {
								self.stream.on_internal_message(internal_message)
									.unwrap_or_else(|e| error!("Raft stream handle internal message error: {}", e));
							}
						}
					},
					Some(in_message) = self.stream.in_rx.next() => {
						self.stream.on_in_message(in_message)
							.unwrap_or_else(|e| error!("Raft stream handle in message error: {}", e));
					},
				}
			}
		}
	}

	fn request_vote(&mut self) -> CommonResult<()> {
		let addresses = &self.stream.authorities.members.clone();
		let term = self.stream.storage.get_current_term();
		let (last_log_index, last_log_term) = self.stream.storage.get_last_log_index_term();
		for address in addresses {
			let req = RequestVoteReq {
				request_id: RequestId(0),
				term,
				last_log_index,
				last_log_term,
			};
			let request_id = self.stream.request_vote(address.clone(), req)?;
			if let Some(request_id) = request_id {
				self.requests.insert(request_id, ());
			}
		}
		Ok(())
	}

	fn on_res_request_vote(&mut self, address: Address, res: RequestVoteRes) -> CommonResult<()> {
		if self.requests.remove(&res.request_id).is_none() {
			return Ok(());
		}

		let current_term = self.stream.storage.get_current_term();
		if res.term > current_term {
			self.stream.update_current_leader(None);
			self.stream.update_state(State::Follower);

			self.stream.storage.update_current_term(res.term)?;
			self.stream.storage.update_current_voted_for(None)?;

			return Ok(());
		}

		if res.vote_granted {
			if self.stream.authorities.members.contains(&address) {
				self.votes_granted += 1;
			}
			debug!(
				"Request vote result: address: {}, votes_granted: {}, votes_needed: {}",
				address, self.votes_granted, self.votes_needed
			);
			if self.votes_granted >= self.votes_needed {
				self.stream.update_state(State::Leader);
				return Ok(());
			}
		}

		Ok(())
	}
}

pub struct FollowerState<'a, S>
where
	S: ConsensusSupport,
{
	stream: &'a mut RaftStream<S>,
}

impl<'a, S> FollowerState<'a, S>
where
	S: ConsensusSupport,
{
	pub fn new(stream: &'a mut RaftStream<S>) -> Self {
		Self { stream }
	}
	pub async fn start(self) -> CommonResult<()> {
		loop {
			if self.stream.state != State::Follower {
				return Ok(());
			}

			// TODO when syncing
			let next_election = sleep_until(self.stream.next_election_instant());
			tokio::select! {
				_ = next_election => {
					self.stream.update_state(State::Candidate);
				},
				Some(internal_message) = self.stream.internal_rx.next() => {
					self.stream.on_internal_message(internal_message)
						.unwrap_or_else(|e| error!("Raft stream handle internal message error: {}", e));
				},
				Some(in_message) = self.stream.in_rx.next() => {
					self.stream.on_in_message(in_message)
						.unwrap_or_else(|e| error!("Raft stream handle in message error: {}", e));
				}
			}
		}
	}
}

pub struct ObserverState<'a, S>
where
	S: ConsensusSupport,
{
	stream: &'a mut RaftStream<S>,
}

impl<'a, S> ObserverState<'a, S>
where
	S: ConsensusSupport,
{
	pub fn new(stream: &'a mut RaftStream<S>) -> Self {
		Self { stream }
	}
	pub async fn start(self) -> CommonResult<()> {
		loop {
			if self.stream.state != State::Observer {
				return Ok(());
			}
			tokio::select! {
				Some(internal_message) = self.stream.internal_rx.next() => {
					self.stream.on_internal_message(internal_message)
						.unwrap_or_else(|e| error!("Raft stream handle internal message error: {}", e));
				},
				Some(in_message) = self.stream.in_rx.next() => {
					self.stream.on_in_message(in_message)
						.unwrap_or_else(|e| error!("Raft stream handle in message error: {}", e));
				}
			}
		}
	}
}

enum ReplicationInMessage {
	AppendEntriesReqResult { request_id: Option<RequestId> },
	AppendEntriesRes { res: AppendEntriesRes },
}

enum ReplicationOutMessage {
	AppendEntriesReq {
		address: Address,
		req: AppendEntriesReq,
	},
	UpdateMatchIndex {
		address: Address,
		match_index: u64,
	},
	UpdateState {
		state: State,
	},
}

struct Replication {
	in_tx: UnboundedSender<ReplicationInMessage>,
	match_index: u64,
}

impl Replication {
	fn new<S>(
		raft_meta: Arc<Meta>,
		out_tx: UnboundedSender<ReplicationOutMessage>,
		storage: Arc<Storage<S>>,
		target: Address,
		match_index: u64,
		match_term: u64,
	) -> CommonResult<Self>
	where
		S: ConsensusSupport,
	{
		let (in_tx, in_rx) = unbounded();

		ReplicationStream::spawn(
			raft_meta,
			out_tx,
			in_rx,
			storage,
			target,
			match_index,
			match_term,
		)?;

		Ok(Self { in_tx, match_index })
	}
}

struct ReplicationStream<S>
where
	S: ConsensusSupport,
{
	#[allow(dead_code)]
	raft_meta: Arc<Meta>,
	out_tx: UnboundedSender<ReplicationOutMessage>,
	in_rx: UnboundedReceiver<ReplicationInMessage>,
	storage: Arc<Storage<S>>,
	target: Address,
	match_index: u64,
	match_term: u64,
	heartbeat: Interval,
	requests: HashMap<RequestId, ()>,
}

impl<S> ReplicationStream<S>
where
	S: ConsensusSupport,
{
	fn spawn(
		raft_meta: Arc<Meta>,
		out_tx: UnboundedSender<ReplicationOutMessage>,
		in_rx: UnboundedReceiver<ReplicationInMessage>,
		storage: Arc<Storage<S>>,
		target: Address,
		match_index: u64,
		match_term: u64,
	) -> CommonResult<()> {
		let heartbeat = interval(Duration::from_millis(raft_meta.heartbeat_interval));
		let this = Self {
			raft_meta,
			out_tx,
			in_rx,
			storage,
			target,
			match_index,
			match_term,
			heartbeat,
			requests: Default::default(),
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) -> CommonResult<()> {
		info!("Start replication work: target: {}", self.target);
		loop {
			tokio::select! {
				_ = self.heartbeat.tick() => {
					self.replicate()?;
				},
				in_message = self.in_rx.next() => {
					match in_message {
						Some(in_message) => {
							self.on_in_message(in_message)
							.unwrap_or_else(|e| error!("Replication stream handle in message error: {}", e))
						},
						None => break,
					}
				}
			}
		}
		Ok(())
	}

	fn on_in_message(&mut self, in_message: ReplicationInMessage) -> CommonResult<()> {
		match in_message {
			ReplicationInMessage::AppendEntriesReqResult { request_id } => {
				self.on_append_entries_req_result(request_id);
			}
			ReplicationInMessage::AppendEntriesRes { res } => {
				self.on_append_entries_res(res)?;
			}
		}
		Ok(())
	}

	fn on_append_entries_req_result(&mut self, request_id: Option<RequestId>) {
		if let Some(request_id) = request_id {
			self.requests.insert(request_id, ());
		}
	}

	fn on_append_entries_res(&mut self, res: AppendEntriesRes) -> CommonResult<()> {
		if self.requests.remove(&res.request_id).is_none() {
			return Ok(());
		}
		if res.term > self.storage.get_current_term() {
			let out_message = ReplicationOutMessage::UpdateState {
				state: State::Follower,
			};
			self.out_tx
				.unbounded_send(out_message)
				.map_err(map_channel_err)?;
			return Ok(());
		}

		let match_index_changed = res.last_log_index != self.match_index;

		self.match_index = res.last_log_index;
		self.match_term = res.last_log_term;

		if match_index_changed {
			let out_message = ReplicationOutMessage::UpdateMatchIndex {
				address: self.target.clone(),
				match_index: self.match_index,
			};
			self.out_tx
				.unbounded_send(out_message)
				.map_err(map_channel_err)?;
		}

		Ok(())
	}

	fn replicate(&self) -> CommonResult<()> {
		let (last_log_index, _last_log_term) = self.storage.get_last_log_index_term();

		let entries = if self.match_index < last_log_index {
			self.storage
				.get_log_entries((self.match_index + 1)..=last_log_index)
		} else {
			vec![]
		};

		let req = AppendEntriesReq {
			request_id: RequestId(0),
			term: self.storage.get_current_term(),
			prev_log_index: self.match_index,
			prev_log_term: self.match_term,
			commit_log_index: self.storage.get_commit_log_index(),
			entries,
		};
		self.append_entries(req)?;
		Ok(())
	}

	fn append_entries(&self, req: AppendEntriesReq) -> CommonResult<()> {
		self.out_tx
			.unbounded_send(ReplicationOutMessage::AppendEntriesReq {
				address: self.target.clone(),
				req,
			})
			.map_err(map_channel_err)?;
		Ok(())
	}
}
