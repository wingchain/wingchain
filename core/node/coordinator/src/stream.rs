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

use std::collections::HashSet;
use std::sync::Arc;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use log::{error, warn};

use node_chain::{ChainCommitBlockParams, ChainOutMessage, CommitBlockResult};
use node_executor::module;
use node_executor_primitives::EmptyParams;
use node_network::{BytesMut, NetworkInMessage, NetworkOutMessage, PMInMessage, PeerId};
use primitives::codec::Decode;
use primitives::errors::CommonResult;
use primitives::{BlockNumber, Body, BuildBlockParams, FullTransaction, Hash, Header, Transaction};

use crate::errors;
use crate::protocol::{
	BlockAnnounce, BlockData, BlockRequest, BlockResponse, BodyData, ProtocolMessage,
};
use crate::support::CoordinatorSupport;
use crate::sync::ChainSync;

pub struct CoordinatorStream<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	chain_rx: UnboundedReceiver<ChainOutMessage>,
	network_rx: UnboundedReceiver<NetworkOutMessage>,
	sync: ChainSync<S>,
	support: Arc<StreamSupport<S>>,
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
			network_rx,
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
				.peer_manager_send_message(PMInMessage::DiscardPeer(peer_id.clone()));
			return Ok(());
		}

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
}

pub struct StreamSupport<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	genesis_hash: Hash,
	peer_manager_tx: UnboundedSender<PMInMessage>,
	network_tx: UnboundedSender<NetworkInMessage>,
	support: Arc<S>,
	system_meta: module::system::Meta,
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
		let system_meta = get_system_meta(support.clone())?;
		Ok(Self {
			genesis_hash,
			peer_manager_tx,
			network_tx,
			support,
			system_meta,
		})
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
			.ok_or(errors::ErrorKind::Data(format!("Missing confirmed number")))?;
		Ok(number)
	}

	pub fn get_block_hash_by_number(&self, number: &BlockNumber) -> CommonResult<Hash> {
		let block_hash = self
			.support
			.get_block_hash(number)?
			.ok_or(errors::ErrorKind::Data(format!(
				"Missing block hash: number: {}",
				number
			)))?;
		Ok(block_hash)
	}

	pub fn get_header_by_block_hash(&self, block_hash: &Hash) -> CommonResult<Header> {
		let header = self
			.support
			.get_header(block_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"Missing header: block_hash: {:?}",
				block_hash
			)))?;
		Ok(header)
	}

	pub fn get_header_by_number(&self, number: &BlockNumber) -> CommonResult<(Hash, Header)> {
		let block_hash = self
			.support
			.get_block_hash(number)?
			.ok_or(errors::ErrorKind::Data(format!(
				"Missing block hash: number: {}",
				number
			)))?;
		let header = self
			.support
			.get_header(&block_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"Missing header: block_hash: {:?}",
				block_hash
			)))?;
		Ok((block_hash, header))
	}

	pub fn get_body_by_block_hash(&self, block_hash: &Hash) -> CommonResult<Body> {
		let body = self
			.support
			.get_body(block_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"Missing body: block_hash: {:?}",
				block_hash
			)))?;
		Ok(body)
	}

	pub fn get_transaction_by_hash(&self, tx_hash: &Hash) -> CommonResult<Transaction> {
		let body = self
			.support
			.get_transaction(tx_hash)?
			.ok_or(errors::ErrorKind::Data(format!(
				"Missing transaction: tx_hash: {:?}",
				tx_hash
			)))?;
		Ok(body)
	}

	pub fn commit_block(
		&self,
		commit_block_params: ChainCommitBlockParams,
	) -> CommonResult<CommitBlockResult> {
		self.support.commit_block(commit_block_params)
	}
}

pub enum VerifyError {
	/// Block duplicated
	Duplicated,
	/// Block is not the best
	NotBest,
	/// Bad block
	Bad,
	/// Invalid execution gap
	InvalidExecutionGap,
	/// Should wait executing
	ShouldWait,
	/// Invalid header
	InvalidHeader(String),
	/// Transaction duplicated
	DuplicatedTx(String),
	/// Transaction invalid
	InvalidTx(String),
}

pub type VerifyResult<T> = Result<T, VerifyError>;

/// verify
impl<S> StreamSupport<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	/// block_data may be taken
	pub fn verify_block(
		&self,
		block_data: &mut Option<BlockData>,
	) -> CommonResult<VerifyResult<ChainCommitBlockParams>> {
		{
			let block_data_ref = block_data.as_ref().expect("qed");
			match self.verify_data(block_data_ref)? {
				Ok(v) => v,
				Err(e) => return Ok(Err(e)),
			};
			let block_hash = &block_data_ref.block_hash;
			let header = block_data_ref.header.as_ref().expect("qed");

			match self.verify_not_repeat(block_hash)? {
				Ok(v) => v,
				Err(e) => return Ok(Err(e)),
			}
			let (_confirmed_number, _confirmed_hash, confirmed_header) =
				match self.verify_best(header)? {
					Ok(v) => v,
					Err(e) => return Ok(Err(e)),
				};

			match self.verify_execution(header, &confirmed_header)? {
				Ok(v) => v,
				Err(e) => return Ok(Err(e)),
			}
		}

		// the following verification need take ownership of block data
		let block_data = block_data.take().expect("qed");
		let confirmed_number = block_data.number - 1;
		let header = block_data.header.expect("qed");
		let body = block_data.body.expect("qed");

		let (meta_txs, payload_txs) = match self.verify_body(body, confirmed_number)? {
			Ok(v) => v,
			Err(e) => return Ok(Err(e)),
		};

		let commit_block_params = match self.verify_header(&header, meta_txs, payload_txs)? {
			Ok(v) => v,
			Err(e) => return Ok(Err(e)),
		};

		Ok(Ok(commit_block_params))
	}

	fn verify_data(&self, block_data: &BlockData) -> CommonResult<VerifyResult<()>> {
		let _header = match &block_data.header {
			Some(v) => v,
			None => return Ok(Err(VerifyError::Bad)),
		};

		let _body = match &block_data.body {
			Some(v) => v,
			None => return Ok(Err(VerifyError::Bad)),
		};

		Ok(Ok(()))
	}

	fn verify_not_repeat(&self, block_hash: &Hash) -> CommonResult<VerifyResult<()>> {
		if self.support.get_header(block_hash)?.is_some() {
			return Ok(Err(VerifyError::Duplicated));
		}
		Ok(Ok(()))
	}

	/// Return confirmed block (number, block hash, header)
	fn verify_best(
		&self,
		header: &Header,
	) -> CommonResult<VerifyResult<(BlockNumber, Hash, Header)>> {
		let confirmed = {
			let confirmed_number = self
				.support
				.get_confirmed_number()?
				.ok_or(errors::ErrorKind::Data(format!("Missing confirmed number")))?;
			let block_hash =
				self.support
					.get_block_hash(&confirmed_number)?
					.ok_or(errors::ErrorKind::Data(format!(
						"Missing block hash: number: {}",
						confirmed_number
					)))?;
			let header = self
				.support
				.get_header(&block_hash)?
				.ok_or(errors::ErrorKind::Data(format!(
					"Missing header: block_hash: {:?}",
					block_hash
				)))?;
			(confirmed_number, block_hash, header)
		};

		if !(header.number == confirmed.0 + 1 && &header.parent_hash == &confirmed.1) {
			return Ok(Err(VerifyError::NotBest));
		}

		Ok(Ok(confirmed))
	}

	fn verify_execution(
		&self,
		header: &Header,
		confirmed_header: &Header,
	) -> CommonResult<VerifyResult<()>> {
		if header.payload_execution_gap < 1 {
			return Ok(Err(VerifyError::InvalidExecutionGap));
		}

		if header.payload_execution_gap > self.system_meta.max_execution_gap {
			return Ok(Err(VerifyError::InvalidExecutionGap));
		}

		// execution number of the verifying block
		let execution_number = header.number - header.payload_execution_gap as u64;

		// execution number of the confirmed block
		let confirmed_execution_number =
			confirmed_header.number - confirmed_header.payload_execution_gap as u64;
		if execution_number < confirmed_execution_number {
			return Ok(Err(VerifyError::InvalidExecutionGap));
		}

		// execution number of current state
		let current_execution_number =
			self.support
				.get_execution_number()?
				.ok_or(errors::ErrorKind::Data(format!(
					"Execution number not found"
				)))?;
		if execution_number > current_execution_number {
			return Ok(Err(VerifyError::ShouldWait));
		}

		Ok(Ok(()))
	}

	/// Return verified txs (meta_txs, payload_txs)
	fn verify_body(
		&self,
		body: BodyData,
		confirmed_number: BlockNumber,
	) -> CommonResult<VerifyResult<(Vec<Arc<FullTransaction>>, Vec<Arc<FullTransaction>>)>> {
		let get_verified_txs =
			|txs: Vec<Transaction>| -> CommonResult<VerifyResult<Vec<Arc<FullTransaction>>>> {
				let mut set = HashSet::new();
				let mut result = Vec::with_capacity(txs.len());
				for tx in txs {
					let tx_hash = self.support.hash_transaction(&tx)?;
					match self.verify_transaction(&tx_hash, &tx, confirmed_number, &mut set)? {
						Ok(()) => (),
						Err(e) => return Ok(Err(e)),
					}
					let tx = Arc::new(FullTransaction { tx_hash, tx });
					result.push(tx);
				}
				Ok(Ok(result))
			};

		let meta_txs = match get_verified_txs(body.meta_txs)? {
			Ok(v) => v,
			Err(e) => return Ok(Err(e)),
		};

		let payload_txs = match get_verified_txs(body.payload_txs)? {
			Ok(v) => v,
			Err(e) => return Ok(Err(e)),
		};

		Ok(Ok((meta_txs, payload_txs)))
	}

	/// return commit block params
	fn verify_header(
		&self,
		header: &Header,
		meta_txs: Vec<Arc<FullTransaction>>,
		payload_txs: Vec<Arc<FullTransaction>>,
	) -> CommonResult<VerifyResult<ChainCommitBlockParams>> {
		let execution_number = header.number - header.payload_execution_gap as u64;

		let build_block_params = BuildBlockParams {
			number: header.number,
			timestamp: header.timestamp,
			meta_txs,
			payload_txs,
			execution_number,
		};

		let commit_block_params = self.support.build_block(build_block_params)?;

		if &commit_block_params.header != header {
			let msg = format!(
				"Invalid header: {:?}, expected: {:?}",
				header, commit_block_params.header
			);
			return Ok(Err(VerifyError::InvalidHeader(msg)));
		}

		Ok(Ok(commit_block_params))
	}

	fn verify_transaction(
		&self,
		tx_hash: &Hash,
		tx: &Transaction,
		confirmed_number: BlockNumber,
		set: &mut HashSet<Hash>,
	) -> CommonResult<VerifyResult<()>> {
		if !set.insert(tx_hash.clone()) {
			return Ok(Err(VerifyError::DuplicatedTx(format!(
				"Duplicated tx: {}",
				tx_hash
			))));
		}

		match self.support.validate_transaction(&tx, true) {
			Ok(()) => (),
			Err(_) => {
				return Ok(Err(VerifyError::InvalidTx(format!(
					"Invalid tx: {}",
					tx_hash
				))));
			}
		};
		let witness = tx.witness.as_ref().expect("qed");

		let max_until_gap = self.system_meta.max_until_gap;
		let max_until = confirmed_number + max_until_gap;

		if witness.until > max_until {
			return Ok(Err(VerifyError::InvalidTx(format!(
				"Invalid until: {}, {}",
				witness.until, tx_hash
			))));
		}

		if witness.until <= confirmed_number {
			return Ok(Err(VerifyError::InvalidTx(format!(
				"Invalid until: {}, {}",
				witness.until, tx_hash
			))));
		}

		if self.support.get_transaction(&tx_hash)?.is_some() {
			return Ok(Err(VerifyError::DuplicatedTx(format!(
				"Duplicated tx: {}",
				tx_hash
			))));
		}

		Ok(Ok(()))
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
		}
	}
}

fn get_system_meta<S: CoordinatorSupport>(support: Arc<S>) -> CommonResult<module::system::Meta> {
	support
		.execute_call_with_block_number(
			&0,
			None,
			"system".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}
