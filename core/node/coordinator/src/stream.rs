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

use std::sync::Arc;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use log::{error, info, warn};

use node_chain::ChainOutMessage;
use node_network::{BytesMut, NetworkInMessage, NetworkOutMessage, PMInMessage, PeerId};
use node_txpool::TxPoolOutMessage;
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::{BlockNumber, Body, Hash, Header, Proof, Transaction};

use crate::peer_report::PEER_REPORT_HANDSHAKE_FAILED;
use crate::protocol::{
	BlockAnnounce, BlockRequest, BlockResponse, ConsensusMessage, ProtocolMessage, TxPropagate,
};
use crate::support::CoordinatorSupport;
use crate::sync::ChainSync;
use crate::{errors, CoordinatorInMessage, DefaultHandshakeBuilder};
use node_consensus_base::{ConsensusInMessage, ConsensusOutMessage};

pub struct CoordinatorStream<S>
where
	S: CoordinatorSupport,
{
	chain_rx: UnboundedReceiver<ChainOutMessage>,
	txpool_rx: UnboundedReceiver<TxPoolOutMessage>,
	network_rx: UnboundedReceiver<NetworkOutMessage>,
	consensus_rx: UnboundedReceiver<ConsensusOutMessage>,
	in_rx: UnboundedReceiver<CoordinatorInMessage>,
	sync: ChainSync<S>,
	support: Arc<StreamSupport<S>>,
}

impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport,
{
	#[allow(clippy::too_many_arguments)]
	pub fn spawn(
		handshake_builder: Arc<DefaultHandshakeBuilder>,
		chain_rx: UnboundedReceiver<ChainOutMessage>,
		txpool_rx: UnboundedReceiver<TxPoolOutMessage>,
		peer_manager_tx: UnboundedSender<PMInMessage>,
		network_tx: UnboundedSender<NetworkInMessage>,
		network_rx: UnboundedReceiver<NetworkOutMessage>,
		consensus_tx: UnboundedSender<ConsensusInMessage>,
		consensus_rx: UnboundedReceiver<ConsensusOutMessage>,
		in_rx: UnboundedReceiver<CoordinatorInMessage>,
		support: Arc<S>,
	) -> CommonResult<()> {
		let support = Arc::new(StreamSupport::new(
			handshake_builder,
			peer_manager_tx,
			network_tx,
			consensus_tx,
			support,
		)?);

		let sync = ChainSync::new(support.clone())?;

		let this = Self {
			chain_rx,
			txpool_rx,
			network_rx,
			consensus_rx,
			in_rx,
			support,
			sync,
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) {
		loop {
			tokio::select! {
				Some(chain_message) = self.chain_rx.next() => {
					self.on_chain_message(chain_message)
						.unwrap_or_else(|e| error!("Coordinator handle chain message error: {}", e));
				}
				Some(network_message) = self.network_rx.next() => {
					self.on_network_message(network_message)
						.unwrap_or_else(|e| error!("Coordinator handle network message error: {}", e));
				}
				Some(in_message) = self.in_rx.next() => {
					self.on_in_message(in_message)
						.unwrap_or_else(|e| error!("Coordinator handle in message error: {}", e));
				}
				Some(txpool_message) = self.txpool_rx.next() => {
					self.on_txpool_message(txpool_message)
						.unwrap_or_else(|e| error!("Coordinator handle txpool message error: {}", e));
				}
				Some(consensus_out_message) = self.consensus_rx.next() => {
					self.on_consensus_out_message(consensus_out_message)
						.unwrap_or_else(|e| error!("Coordinator handle consensus message error: {}", e));
				}
				Some(block_request_timer_result) = self.sync.block_request_timer.next() => {
					let (peer_id, request_id) = block_request_timer_result;
					self.sync.on_block_request_timer_trigger(peer_id, request_id)
						.unwrap_or_else(|e| error!("Coordinator handle block request timer result error: {}", e));
				}
			}
		}
	}

	fn on_chain_message(&mut self, message: ChainOutMessage) -> CommonResult<()> {
		match message {
			ChainOutMessage::BlockCommitted { number, hash } => {
				self.on_block_committed(number, hash)
			}
			ChainOutMessage::ExecutionCommitted { number, hash } => {
				self.on_execution_committed(number, hash)
			}
		}
	}

	fn on_txpool_message(&mut self, message: TxPoolOutMessage) -> CommonResult<()> {
		match message {
			TxPoolOutMessage::TxInserted { tx_hash } => self.on_tx_inserted(tx_hash),
		}
	}

	fn on_network_message(&mut self, message: NetworkOutMessage) -> CommonResult<()> {
		match message {
			NetworkOutMessage::ProtocolOpen {
				peer_id,
				handshake,
				nonce,
				..
			} => self.on_protocol_open(peer_id, nonce, handshake),
			NetworkOutMessage::ProtocolClose { peer_id, .. } => self.on_protocol_close(peer_id),
			NetworkOutMessage::Message { peer_id, message } => self.on_message(peer_id, message),
		}
	}

	fn on_in_message(&mut self, message: CoordinatorInMessage) -> CommonResult<()> {
		match message {
			CoordinatorInMessage::Network(message) => self.support.network_send_message(message),
		}
		Ok(())
	}
}

/// methods for chain messages
impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport,
{
	fn on_block_committed(&mut self, number: BlockNumber, hash: Hash) -> CommonResult<()> {
		self.sync.on_block_committed(number, hash)?;
		Ok(())
	}

	fn on_execution_committed(&mut self, number: BlockNumber, hash: Hash) -> CommonResult<()> {
		self.sync.on_execution_committed(number, hash)?;
		Ok(())
	}
}

/// methods for network messages
impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport,
{
	fn on_protocol_open(
		&mut self,
		peer_id: PeerId,
		nonce: u64,
		handshake: Vec<u8>,
	) -> CommonResult<()> {
		let handshake = match Decode::decode(&mut &handshake[..]) {
			Ok(ProtocolMessage::Handshake(handshake)) => {
				let local_genesis_hash = &self.support.get_handshake_builder().genesis_hash;
				if &handshake.genesis_hash == local_genesis_hash {
					Some(handshake)
				} else {
					warn!(
						"Handshake from {} is different: local: {}, remote: {}",
						peer_id, local_genesis_hash, handshake.genesis_hash
					);
					None
				}
			}
			Ok(_) => {
				warn!("Handshake from {} is invalid", peer_id);
				None
			}
			Err(e) => {
				warn!("Handshake from {} cannot decode: {:?}", peer_id, e);
				None
			}
		};
		if handshake.is_none() {
			warn!("Report {} for handshake failure", peer_id);
			self.support
				.peer_manager_send_message(PMInMessage::ReportPeer(
					peer_id,
					PEER_REPORT_HANDSHAKE_FAILED,
				));
			return Ok(());
		}
		let handshake = handshake.expect("qed");
		info!(
			"Complete handshake with {}: nonce: {}, handshake: {:?}",
			peer_id, nonce, handshake
		);

		let remote_nonce = handshake.nonce;
		self.sync
			.on_protocol_open(peer_id.clone(), nonce, handshake)?;
		self.on_consensus_network_protocol_open(peer_id, nonce, remote_nonce)?;

		Ok(())
	}

	fn on_protocol_close(&mut self, peer_id: PeerId) -> CommonResult<()> {
		self.sync.on_protocol_close(peer_id.clone())?;
		self.on_consensus_network_protocol_close(peer_id)?;
		Ok(())
	}

	fn on_message(&mut self, peer_id: PeerId, message: BytesMut) -> CommonResult<()> {
		let message: ProtocolMessage = match Decode::decode(&mut message.as_ref()) {
			Ok(message) => message,
			Err(e) => {
				warn!("Message from {} cannot decode: {:?}", peer_id, e);
				return Ok(());
			}
		};

		match message {
			ProtocolMessage::BlockAnnounce(block_announce) => {
				self.on_block_announce(peer_id, block_announce)
			}
			ProtocolMessage::BlockRequest(block_request) => {
				self.on_block_request(peer_id, block_request)
			}
			ProtocolMessage::BlockResponse(block_response) => {
				self.on_block_response(peer_id, block_response)
			}
			ProtocolMessage::TxPropagate(tx_propagate) => {
				self.on_tx_propagate(peer_id, tx_propagate)
			}
			ProtocolMessage::ConsensusMessage(consensus_message) => {
				self.on_consensus_network_message(peer_id, consensus_message)
			}
			ProtocolMessage::Handshake(_) => Ok(()),
		}
	}

	fn on_block_announce(
		&mut self,
		peer_id: PeerId,
		block_announce: BlockAnnounce,
	) -> CommonResult<()> {
		self.sync.on_block_announce(peer_id, block_announce)
	}

	fn on_block_request(
		&mut self,
		peer_id: PeerId,
		block_request: BlockRequest,
	) -> CommonResult<()> {
		self.sync.on_block_request(peer_id, block_request)
	}

	fn on_block_response(
		&mut self,
		peer_id: PeerId,
		block_response: BlockResponse,
	) -> CommonResult<()> {
		self.sync.on_block_response(peer_id, block_response)
	}

	fn on_tx_propagate(&mut self, peer_id: PeerId, tx_propagate: TxPropagate) -> CommonResult<()> {
		self.sync.on_tx_propagate(peer_id, tx_propagate)
	}
}

/// methods for txpool messages
impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport,
{
	fn on_tx_inserted(&mut self, tx_hash: Hash) -> CommonResult<()> {
		self.sync.on_tx_inserted(tx_hash)
	}
}

/// methods for consensus messages
impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport,
{
	fn on_consensus_network_protocol_open(
		&mut self,
		peer_id: PeerId,
		local_nonce: u64,
		remote_nonce: u64,
	) -> CommonResult<()> {
		let in_message = ConsensusInMessage::NetworkProtocolOpen {
			peer_id,
			local_nonce,
			remote_nonce,
		};
		self.support.consensus_send_message(in_message);
		Ok(())
	}

	fn on_consensus_network_protocol_close(&mut self, peer_id: PeerId) -> CommonResult<()> {
		let in_message = ConsensusInMessage::NetworkProtocolClose { peer_id };
		self.support.consensus_send_message(in_message);
		Ok(())
	}

	fn on_consensus_network_message(
		&mut self,
		peer_id: PeerId,
		consensus_message: ConsensusMessage,
	) -> CommonResult<()> {
		let in_message = ConsensusInMessage::NetworkMessage {
			peer_id,
			message: consensus_message.message,
		};
		self.support.consensus_send_message(in_message);
		Ok(())
	}

	fn on_consensus_out_message(&self, out_message: ConsensusOutMessage) -> CommonResult<()> {
		match out_message {
			ConsensusOutMessage::NetworkMessage { peer_id, message } => {
				self.on_consensus_out_network_message(peer_id, message)?;
			}
		}
		Ok(())
	}

	fn on_consensus_out_network_message(
		&self,
		peer_id: PeerId,
		message: Vec<u8>,
	) -> CommonResult<()> {
		let message = ProtocolMessage::ConsensusMessage(ConsensusMessage { message });
		self.support
			.network_send_message(NetworkInMessage::SendMessage {
				peer_id,
				message: message.encode(),
			});
		Ok(())
	}
}

pub struct StreamSupport<S>
where
	S: CoordinatorSupport,
{
	handshake_builder: Arc<DefaultHandshakeBuilder>,
	peer_manager_tx: UnboundedSender<PMInMessage>,
	network_tx: UnboundedSender<NetworkInMessage>,
	consensus_tx: UnboundedSender<ConsensusInMessage>,
	support: Arc<S>,
}

impl<S> StreamSupport<S>
where
	S: CoordinatorSupport,
{
	pub fn new(
		handshake_builder: Arc<DefaultHandshakeBuilder>,
		peer_manager_tx: UnboundedSender<PMInMessage>,
		network_tx: UnboundedSender<NetworkInMessage>,
		consensus_tx: UnboundedSender<ConsensusInMessage>,
		support: Arc<S>,
	) -> CommonResult<Self> {
		Ok(Self {
			handshake_builder,
			peer_manager_tx,
			network_tx,
			consensus_tx,
			support,
		})
	}

	pub fn ori_support(&self) -> Arc<S> {
		self.support.clone()
	}

	pub fn peer_manager_send_message(&self, message: PMInMessage) {
		self.peer_manager_tx
			.unbounded_send(message)
			.unwrap_or_else(|e| error!("Coordinator send message to peer manager error: {}", e));
	}

	pub fn network_send_message(&self, message: NetworkInMessage) {
		self.network_tx
			.unbounded_send(message)
			.unwrap_or_else(|e| error!("Coordinator send message to network error: {}", e));
	}

	pub fn consensus_send_message(&self, message: ConsensusInMessage) {
		self.consensus_tx
			.unbounded_send(message)
			.unwrap_or_else(|e| error!("Coordinator send message to consensus error: {}", e));
	}

	pub fn get_handshake_builder(&self) -> &Arc<DefaultHandshakeBuilder> {
		&self.handshake_builder
	}

	pub fn get_block_hash_by_number(&self, number: &BlockNumber) -> CommonResult<Hash> {
		let block_hash = self.support.get_block_hash(number)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing block hash: number: {}", number))
		})?;
		Ok(block_hash)
	}

	pub fn get_header_by_block_hash(&self, block_hash: &Hash) -> CommonResult<Header> {
		let header = self.support.get_header(block_hash)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing header: block_hash: {:?}", block_hash))
		})?;
		Ok(header)
	}

	pub fn get_proof_by_block_hash(&self, block_hash: &Hash) -> CommonResult<Proof> {
		let header = self.support.get_proof(block_hash)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing proof: block_hash: {:?}", block_hash))
		})?;
		Ok(header)
	}

	pub fn get_body_by_block_hash(&self, block_hash: &Hash) -> CommonResult<Body> {
		let body = self.support.get_body(block_hash)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing body: block_hash: {:?}", block_hash))
		})?;
		Ok(body)
	}

	pub fn get_transaction_by_hash(&self, tx_hash: &Hash) -> CommonResult<Transaction> {
		let body = self.support.get_transaction(tx_hash)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing transaction: tx_hash: {:?}", tx_hash))
		})?;
		Ok(body)
	}
}
