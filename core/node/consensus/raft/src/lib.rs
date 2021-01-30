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
use node_consensus_base::errors::ErrorKind;

pub mod errors;
mod protocol;
mod request;

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
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) {
		info!("Start raft work");
		loop {
			tokio::select! {
				Some(in_message) = self.in_rx.next() => {
					self.on_in_message(in_message).await
						.unwrap_or_else(|e| error!("Consensus raft handle in message error: {}", e));
				}
			}
		}
	}
}

/// methods for in messages
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	async fn on_in_message(&mut self, in_message: ConsensusInMessage) -> CommonResult<()> {
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
}

/// methods for out messages
impl<S> RaftStream<S>
where
	S: ConsensusSupport,
{
	fn send_out_message(&self, out_message: ConsensusOutMessage) -> CommonResult<()> {
		self.out_tx
			.unbounded_send(out_message)
			.map_err(|e| ErrorKind::Channel(Box::new(e)))?;
		Ok(())
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
