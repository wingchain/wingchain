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

//! Raft consensus
//! TODO rm
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::prelude::*;
use log::{error, info};
use parking_lot::RwLock;

use crypto::address::Address as AddressT;
use crypto::dsa::{Dsa, KeyPair};
use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{
	Consensus as ConsensusT, ConsensusConfig, ConsensusInMessage, ConsensusOutMessage, PeerId,
};
use node_consensus_primitives::CONSENSUS_RAFT;
use node_executor::module::{self, raft::Meta};
use node_executor_primitives::EmptyParams;
use primitives::errors::CommonResult;
use primitives::{Address, BlockNumber, Header, PublicKey, SecretKey};

use crate::protocol::RequestId;
use crate::state::{LogIndex, Term};

mod chain;
pub mod errors;
mod network;
mod protocol;
mod state;

pub struct Raft<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
	in_tx: UnboundedSender<ConsensusInMessage>,
	out_rx: RwLock<Option<UnboundedReceiver<ConsensusOutMessage>>>,
}

impl<S> Raft<S>
where
	S: ConsensusSupport,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let raft_meta = get_raft_meta(&support, &0)?;

		let secret_key = config.secret_key;

		let (in_tx, in_rx) = unbounded();
		let (out_tx, out_rx) = unbounded();

		RaftStream::spawn(support.clone(), raft_meta, secret_key, out_tx, in_rx)?;

		info!("Initializing consensus raft");

		let raft = Raft {
			support,
			in_tx,
			out_rx: RwLock::new(Some(out_rx)),
		};

		Ok(raft)
	}
}

impl<S> ConsensusT for Raft<S>
where
	S: ConsensusSupport,
{
	fn verify_proof(&self, _header: &Header, proof: &primitives::Proof) -> CommonResult<()> {
		let name = &proof.name;
		if name != CONSENSUS_RAFT {
			return Err(
				node_consensus_base::errors::ErrorKind::VerifyProofError(format!(
					"Unexpected consensus: {}",
					name
				))
				.into(),
			);
		}
		unimplemented!()
	}

	fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		self.in_tx.clone()
	}

	fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		self.out_rx.write().take()
	}
}

struct PeerInfo {
	local_nonce: u64,
	remote_nonce: u64,
	address: Option<Address>,
}

#[allow(clippy::type_complexity)]
struct RaftStream<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
	raft_meta: Meta,
	secret_key: SecretKey,
	public_key: PublicKey,
	address: Address,
	out_tx: UnboundedSender<ConsensusOutMessage>,
	in_rx: UnboundedReceiver<ConsensusInMessage>,
	peers: HashMap<PeerId, PeerInfo>,
	known_validators: HashMap<Address, PeerId>,
	next_request_id: RequestId,
	last_log_index: LogIndex,
	last_log_term: Term,
	current_term: Term,
}

impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	fn spawn(
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
			last_log_index: Default::default(),
			last_log_term: Default::default(),
			current_term: Default::default(),
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) -> CommonResult<()> {
		info!("Start raft work");

		self.last_log_index = self.chain_get_last_log_index()?;
		self.last_log_term = self.chain_get_last_log_term()?;
		self.current_term = self.chain_get_current_term()?;

		loop {
			tokio::select! {
				Some(in_message) = self.in_rx.next() => {
					self.on_in_message(in_message)
						.unwrap_or_else(|e| error!("Consensus raft handle in message error: {}", e));
				}
			}
		}
	}
}

fn get_raft_meta<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<module::raft::Meta> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"raft".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
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
