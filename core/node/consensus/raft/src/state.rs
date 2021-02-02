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

use futures::prelude::*;
use log::error;
use tokio::time::{interval, sleep_until, Duration, Interval};

use crate::protocol::RequestVoteRes;
use crate::stream::RaftStream;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use log::{debug, info};
use node_consensus_base::support::ConsensusSupport;
use node_executor::module::raft::Meta;
use primitives::errors::CommonResult;
use primitives::Address;
use std::sync::Arc;

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
}

impl<'a, S> LeaderState<'a, S>
where
	S: ConsensusSupport,
{
	pub fn new(stream: &'a mut RaftStream<S>) -> Self {
		Self { stream }
	}
	pub async fn start(self) -> CommonResult<()> {
		unimplemented!()
	}
}

pub struct CandidateState<'a, S>
where
	S: ConsensusSupport,
{
	stream: &'a mut RaftStream<S>,
	votes_granted: u64,
	votes_needed: u64,
	request_vote_res_rx: UnboundedReceiver<(Address, RequestVoteRes)>,
}

impl<'a, S> CandidateState<'a, S>
where
	S: ConsensusSupport,
{
	pub fn new(stream: &'a mut RaftStream<S>) -> Self {
		let (tx, rx) = unbounded();
		stream.request_vote_res_tx = Some(tx);

		Self {
			stream,
			votes_granted: 0,
			votes_needed: 0,
			request_vote_res_rx: rx,
		}
	}
	pub async fn start(mut self) -> CommonResult<()> {
		loop {
			if self.stream.state != State::Candidate {
				return Ok(());
			}
			self.votes_granted = 1;
			self.votes_needed = ((self.stream.storage.get_authorities_len() / 2) + 1) as u64;

			self.stream.update_next_election_instant(false);
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
					Some(in_message) = self.stream.in_rx.next() => {
						self.stream.on_in_message(in_message)
							.unwrap_or_else(|e| error!("Consensus raft handle in message error: {}", e));
					},
					Some((address, res)) = self.request_vote_res_rx.next() => {
						self.on_res_request_vote(address, res)
							.unwrap_or_else(|e| error!("Consensus raft handle request vote res error: {}", e));
					},
				}
			}
		}
	}

	fn request_vote(&mut self) -> CommonResult<()> {
		let addresses = self.stream.storage.get_authorities();
		for address in addresses {
			self.stream.request_vote(address)?
		}
		Ok(())
	}

	fn on_res_request_vote(&mut self, address: Address, res: RequestVoteRes) -> CommonResult<()> {
		let current_term = self.stream.storage.get_current_term();
		if res.term > current_term {
			self.stream.update_current_leader(None);
			self.stream.update_state(State::Follower);

			self.stream.storage.update_current_term(res.term)?;
			self.stream.storage.update_current_voted_for(None)?;

			return Ok(());
		}

		if res.vote_granted {
			if self.stream.storage.authorities_contains(&address) {
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

			let next_election = sleep_until(self.stream.next_election_instant());
			tokio::select! {
				_ = next_election => {
					self.stream.update_state(State::Candidate);
				},
				Some(in_message) = self.stream.in_rx.next() => {
					self.stream.on_in_message(in_message)
						.unwrap_or_else(|e| error!("Consensus raft handle in message error: {}", e));
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
		unimplemented!()
	}
}

enum ReplicationInMessage {}

enum ReplicationOutMessage {}

struct ReplicationStream {
	raft_meta: Arc<Meta>,
	out_tx: UnboundedSender<ReplicationOutMessage>,
	in_rx: UnboundedReceiver<ReplicationInMessage>,
	heartbeat: Interval,
	target: Address,
}

impl ReplicationStream {
	fn spawn(
		raft_meta: Arc<Meta>,
		out_tx: UnboundedSender<ReplicationOutMessage>,
		in_rx: UnboundedReceiver<ReplicationInMessage>,
		target: Address,
	) -> CommonResult<()> {
		let heartbeat = interval(Duration::from_millis(raft_meta.heartbeat_interval));
		let this = Self {
			raft_meta,
			out_tx,
			in_rx,
			heartbeat,
			target,
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) -> CommonResult<()> {
		info!("Start replication work");
		tokio::select! {
			_ = self.heartbeat.tick() => {

			},
			Some(_in_message) = self.in_rx.next() => {

			}
		}
		Ok(())
	}
}
