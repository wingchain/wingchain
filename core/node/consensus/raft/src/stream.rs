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

use crate::network::PeerInfo;
use crate::protocol::{AppendEntriesRes, RequestId, RequestVoteRes};
use crate::state::{CandidateState, FollowerState, LeaderState, ObserverState, State};
use crate::storage::Storage;
use crypto::address::Address as AddressT;
use crypto::dsa::{Dsa, KeyPair};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use log::info;
use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{ConsensusInMessage, ConsensusOutMessage, PeerId};
use node_executor::module::raft::Meta;
use primitives::errors::CommonResult;
use primitives::{Address, PublicKey, SecretKey};
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Duration;
use tokio::time::Instant;

const INIT_EXTRA_ELECTION_TIMEOUT: u64 = 10000;

#[allow(clippy::type_complexity)]
pub struct RaftStream<S>
where
	S: ConsensusSupport,
{
	pub support: Arc<S>,
	pub raft_meta: Arc<Meta>,
	pub secret_key: SecretKey,
	pub public_key: PublicKey,
	pub address: Address,
	pub out_tx: UnboundedSender<ConsensusOutMessage>,
	pub in_rx: UnboundedReceiver<ConsensusInMessage>,
	pub peers: HashMap<PeerId, PeerInfo>,
	pub known_validators: HashMap<Address, PeerId>,
	pub next_request_id: RequestId,
	pub storage: Arc<Storage<S>>,
	pub current_leader: Option<Address>,
	pub next_election_instant: Option<Instant>,
	pub last_heartbeat_instant: Option<Instant>,
	pub state: State,
	pub request_vote_res_tx: Option<UnboundedSender<(Address, RequestVoteRes)>>,
	pub append_entries_res_tx: Option<UnboundedSender<(Address, AppendEntriesRes)>>,
}

impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	pub fn spawn(
		support: Arc<S>,
		raft_meta: Meta,
		secret_key: Option<SecretKey>,
		out_tx: UnboundedSender<ConsensusOutMessage>,
		in_rx: UnboundedReceiver<ConsensusInMessage>,
	) -> CommonResult<()> {
		let secret_key = match secret_key {
			Some(v) => v,
			None => return Ok(()),
		};

		let public_key = get_public_key(&secret_key, &support)?;
		let address = get_address(&public_key, &support)?;
		let raft_meta = Arc::new(raft_meta);
		let storage = Arc::new(Storage::new(support.clone())?);

		let this = Self {
			support,
			raft_meta,
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
			request_vote_res_tx: None,
			append_entries_res_tx: None,
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) -> CommonResult<()> {
		info!("Start raft work");

		self.update_state(State::Follower);
		if self.state == State::Follower {
			let next_election_instant = Instant::now()
				+ Duration::from_millis(self.rand_election_timeout() + INIT_EXTRA_ELECTION_TIMEOUT);
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

	pub fn next_election_instant(&mut self) -> Instant {
		match self.next_election_instant {
			Some(inst) => inst,
			None => {
				let inst = Instant::now() + Duration::from_millis(self.rand_election_timeout());
				self.next_election_instant = Some(inst);
				inst
			}
		}
	}

	pub fn update_next_election_instant(&mut self, heartbeat: bool) {
		let now = Instant::now();
		self.next_election_instant =
			Some(now + Duration::from_millis(self.rand_election_timeout()));
		if heartbeat {
			self.last_heartbeat_instant = Some(now);
		}
	}

	pub fn update_current_leader(&mut self, leader: Option<Address>) {
		self.current_leader = leader;
	}

	pub fn update_state(&mut self, state: State) {
		if state == State::Follower && !self.storage.authorities_contains(&self.address) {
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
}

pub fn get_public_key<S: ConsensusSupport>(
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

pub fn get_address<S: ConsensusSupport>(
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
