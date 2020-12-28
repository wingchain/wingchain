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

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use log::{error, warn};

use node_chain::ChainOutMessage;
use node_network::{BytesMut, NetworkInMessage, NetworkOutMessage, PMInMessage, PeerId};
use primitives::codec::{Decode, Encode};
use primitives::errors::CommonResult;
use primitives::types::FullHeader;
use primitives::{BlockNumber, Hash};

use crate::errors;
use crate::protocol::{BlockAnnounce, ProtocolMessage};
use crate::support::CoordinatorSupport;
use lru::LruCache;

const PEER_KNOWN_BLOCKS_SIZE: usize = 1024;

pub struct CoordinatorStream<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	genesis_hash: Hash,
	chain_rx: UnboundedReceiver<ChainOutMessage>,
	peer_manager_tx: UnboundedSender<PMInMessage>,
	network_tx: UnboundedSender<NetworkInMessage>,
	network_rx: UnboundedReceiver<NetworkOutMessage>,
	support: Arc<S>,
	peers: HashMap<PeerId, PeerInfo>,
}

pub struct PeerInfo {
	known_blocks: LruCache<Hash, ()>,
	confirmed_number: BlockNumber,
	confirmed_hash: Hash,
}

impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	pub fn new(
		genesis_hash: Hash,
		chain_rx: UnboundedReceiver<ChainOutMessage>,
		peer_manager_tx: UnboundedSender<PMInMessage>,
		network_tx: UnboundedSender<NetworkInMessage>,
		network_rx: UnboundedReceiver<NetworkOutMessage>,
		support: Arc<S>,
	) -> Self {
		Self {
			genesis_hash,
			chain_rx,
			peer_manager_tx,
			network_tx,
			network_rx,
			support,
			peers: HashMap::new(),
		}
	}

	fn on_chain_message(&mut self, message: ChainOutMessage) -> CommonResult<()> {
		match message {
			ChainOutMessage::BlockCommitted { number, hash } => self.on_block_committed(number, hash),
			_ => Ok(()),
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

}

/// methods for chain messages
impl<S> CoordinatorStream<S>
	where
		S: CoordinatorSupport + Send + Sync + 'static,
{
	fn on_block_committed(&mut self, _number: BlockNumber, hash: Hash) -> CommonResult<()> {
		let (block_hash, block_announce) = {
			let header = self.get_full_header_by_block_hash(hash)?;
			let block_hash = header.block_hash.clone();
			(block_hash, ProtocolMessage::BlockAnnounce(BlockAnnounce { header }))
		};
		let block_announce = block_announce.encode();
		let network_tx = &self.network_tx;
		for (peer_id, peer_info) in &mut self.peers {

			peer_info.known_blocks.put(block_hash.clone(), ());
			Self::network_send_message(&network_tx, NetworkInMessage::SendMessage {
				peer_id: peer_id.clone(),
				message: block_announce.clone(),
			});
		}
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
				let ok = handshake.genesis_hash == self.genesis_hash;
				if !ok {
					warn!(
						"Handshake from {} is different: local: {}, remote: {}",
						peer_id, self.genesis_hash, handshake.genesis_hash
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
			Self::peer_manager_send_message(&self.peer_manager_tx, PMInMessage::DiscardPeer(peer_id.clone()));
			return Ok(());
		}

		self.peers.insert(peer_id.clone(), PeerInfo {
			known_blocks: LruCache::new(PEER_KNOWN_BLOCKS_SIZE),
			confirmed_number: 0,
			confirmed_hash: self.genesis_hash.clone(),
		});

		// announce block
		let (block_hash, block_announce) = {
			let header = self.get_full_header_by_number(self.get_confirmed_number()?)?;
			let block_hash = header.block_hash.clone();
			(block_hash, ProtocolMessage::BlockAnnounce(BlockAnnounce { header }))
		};
		match self.peers.get_mut(&peer_id) {
			Some(peer) => {
				peer.known_blocks.put(block_hash, ());
			},
			_ => (),
		}
		Self::network_send_message(&self.network_tx,NetworkInMessage::SendMessage {
			peer_id: peer_id,
			message: block_announce.encode(),
		});

		Ok(())
	}

	fn on_protocol_close(&mut self, peer_id: PeerId) -> CommonResult<()> {
		self.peers.remove(&peer_id);
		Ok(())
	}

	fn on_message(&self, _peer_id: PeerId, _message: BytesMut) -> CommonResult<()> {
		unimplemented!()
	}
}

/// methods for util
impl<S> CoordinatorStream<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	fn peer_manager_send_message(tx: &UnboundedSender<PMInMessage>, message: PMInMessage) {
		tx.unbounded_send(message)
			.unwrap_or_else(|e| error!("Coordinator send message to peer manager error: {}", e));
	}

	fn network_send_message(tx: &UnboundedSender<NetworkInMessage>, message: NetworkInMessage) {
		tx.unbounded_send(message)
			.unwrap_or_else(|e| error!("Coordinator send message to network error: {}", e));
	}

	fn get_confirmed_number(&self) -> CommonResult<BlockNumber> {
		let number = self
			.support
			.get_confirmed_number()?
			.ok_or(errors::ErrorKind::Data(format!("missing confirmed number")))?;
		Ok(number)
	}
	fn get_full_header_by_block_hash(&self, block_hash: Hash) -> CommonResult<FullHeader> {
		let header = self
			.support
			.get_header(&block_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"missing header: block_hash: {:?}",
				block_hash
			)))?;
		let header = FullHeader { header, block_hash };
		Ok(header)
	}
	fn get_full_header_by_number(&self, number: BlockNumber) -> CommonResult<FullHeader> {
		let block_hash = self
			.support
			.get_block_hash(&number)?
			.ok_or(errors::ErrorKind::Data(format!(
				"missing block hash: number: {}",
				number
			)))?;
		let header = self
			.support
			.get_header(&block_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"missing header: block_hash: {:?}",
				block_hash
			)))?;
		let header = FullHeader { header, block_hash };
		Ok(header)
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
					.unwrap_or_else(|e| error!("Coordinator handle chain error: {}", e));
			}
			network_message = stream.network_rx.next() => {
				let network_message = match network_message {
					Some(v) => v,
					None => return,
				};
				stream.on_network_message(network_message)
					.unwrap_or_else(|e| error!("Coordinator handle network error: {}", e));
			}
		}
	}
}
