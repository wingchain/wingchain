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
use primitives::codec::Decode;
use primitives::errors::CommonResult;
use primitives::{BlockNumber, Body, Hash, Header, Transaction};

use crate::protocol::{BlockAnnounce, BlockRequest, BlockResponse, ProtocolMessage, TxPropagate};
use crate::support::CoordinatorSupport;
use crate::sync::ChainSync;
use crate::{errors, CoordinatorInMessage};

pub struct CoordinatorStream<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	chain_rx: UnboundedReceiver<ChainOutMessage>,
	txpool_rx: UnboundedReceiver<TxPoolOutMessage>,
	network_rx: UnboundedReceiver<NetworkOutMessage>,
	in_rx: UnboundedReceiver<CoordinatorInMessage>,
	sync: ChainSync<S>,
	support: Arc<StreamSupport<S>>,
}

impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	#[allow(clippy::too_many_arguments)]
	pub fn new(
		genesis_hash: Hash,
		chain_rx: UnboundedReceiver<ChainOutMessage>,
		txpool_rx: UnboundedReceiver<TxPoolOutMessage>,
		peer_manager_tx: UnboundedSender<PMInMessage>,
		network_tx: UnboundedSender<NetworkInMessage>,
		network_rx: UnboundedReceiver<NetworkOutMessage>,
		in_rx: UnboundedReceiver<CoordinatorInMessage>,
		support: Arc<S>,
	) -> CommonResult<Self> {
		let support = Arc::new(StreamSupport::new(
			genesis_hash,
			peer_manager_tx,
			network_tx,
			support,
		)?);

		let sync = ChainSync::new(support.clone())?;

		let stream = Self {
			chain_rx,
			txpool_rx,
			network_rx,
			in_rx,
			support,
			sync,
		};
		Ok(stream)
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
				peer_id, handshake, ..
			} => self.on_protocol_open(peer_id, handshake),
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
	S: CoordinatorSupport + Send + Sync + 'static,
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
	S: CoordinatorSupport + Send + Sync + 'static,
{
	fn on_protocol_open(&mut self, peer_id: PeerId, handshake: Vec<u8>) -> CommonResult<()> {
		let handshake_ok = match Decode::decode(&mut &handshake[..]) {
			Ok(ProtocolMessage::Handshake(handshake)) => {
				let local_genesis_hash = self.support.get_genesis_hash();
				let ok = &handshake.genesis_hash == local_genesis_hash;
				if !ok {
					warn!(
						"Handshake from {} is different: local: {}, remote: {}",
						peer_id, local_genesis_hash, handshake.genesis_hash
					);
				}
				ok
			}
			Ok(_) => {
				warn!("Handshake from {} is invalid", peer_id);
				false
			}
			Err(e) => {
				warn!("Handshake from {} cannot decode: {:?}", peer_id, e);
				false
			}
		};
		if !handshake_ok {
			warn!("Discard {} for handshake failure", peer_id);
			self.support
				.peer_manager_send_message(PMInMessage::DiscardPeer(peer_id));
			return Ok(());
		}
		info!("Complete handshake with {}", peer_id);

		self.sync.on_protocol_open(peer_id)?;

		Ok(())
	}

	fn on_protocol_close(&mut self, peer_id: PeerId) -> CommonResult<()> {
		self.sync.on_protocol_close(peer_id)?;
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
	S: CoordinatorSupport + Send + Sync + 'static,
{
	fn on_tx_inserted(&mut self, tx_hash: Hash) -> CommonResult<()> {
		self.sync.on_tx_inserted(tx_hash)
	}
}

pub struct StreamSupport<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	genesis_hash: Hash,
	peer_manager_tx: UnboundedSender<PMInMessage>,
	network_tx: UnboundedSender<NetworkInMessage>,
	support: Arc<S>,
}

impl<S> StreamSupport<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	pub fn new(
		genesis_hash: Hash,
		peer_manager_tx: UnboundedSender<PMInMessage>,
		network_tx: UnboundedSender<NetworkInMessage>,
		support: Arc<S>,
	) -> CommonResult<Self> {
		Ok(Self {
			genesis_hash,
			peer_manager_tx,
			network_tx,
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

	pub fn get_genesis_hash(&self) -> &Hash {
		&self.genesis_hash
	}

	pub fn get_confirmed_number(&self) -> CommonResult<BlockNumber> {
		let number = self
			.support
			.get_confirmed_number()?
			.ok_or_else(|| errors::ErrorKind::Data("Missing confirmed number".to_string()))?;
		Ok(number)
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

	pub fn get_header_by_number(&self, number: &BlockNumber) -> CommonResult<(Hash, Header)> {
		let block_hash = self.support.get_block_hash(number)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing block hash: number: {}", number))
		})?;
		let header = self.support.get_header(&block_hash)?.ok_or_else(|| {
			errors::ErrorKind::Data(format!("Missing header: block_hash: {:?}", block_hash))
		})?;
		Ok((block_hash, header))
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

pub async fn start<S>(mut stream: CoordinatorStream<S>)
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	loop {
		futures::select! {
			chain_message = stream.chain_rx.next() => {
				let chain_message = match chain_message {
					Some(v) => v,
					None => return,
				};
				stream.on_chain_message(chain_message)
					.unwrap_or_else(|e| error!("Coordinator handle chain message error: {}", e));
			}
			network_message = stream.network_rx.next() => {
				let network_message = match network_message {
					Some(v) => v,
					None => return,
				};
				stream.on_network_message(network_message)
					.unwrap_or_else(|e| error!("Coordinator handle network message error: {}", e));
			}
			in_message = stream.in_rx.next() => {
				let in_message = match in_message {
					Some(v) => v,
					None => return,
				};
				stream.on_in_message(in_message)
					.unwrap_or_else(|e| error!("Coordinator handle in message error: {}", e));
			}
			txpool_message = stream.txpool_rx.next() => {
				let txpool_message = match txpool_message {
					Some(v) => v,
					None => return,
				};
				stream.on_txpool_message(txpool_message)
					.unwrap_or_else(|e| error!("Coordinator handle txpool message error: {}", e));
			}
		}
	}
}
