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

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use log::{info, trace};
use lru::LruCache;

use node_chain::ChainCommitBlockParams;
use node_network::{NetworkInMessage, PMInMessage, PeerId};
use primitives::codec::Encode;
use primitives::errors::{Catchable, CommonResult};
use primitives::{BlockNumber, FullTransaction, Hash, Header};

use crate::errors::ErrorKind;
use crate::peer_report::{
	PEER_REPORT_BLOCK_REQUEST_TIMEOUT, PEER_REPORT_INVALID_BLOCK, PEER_REPORT_INVALID_TX,
};
use crate::protocol::{
	BlockAnnounce, BlockData, BlockId, BlockRequest, BlockResponse, BodyData, Direction,
	ProtocolMessage, RequestId, TxPropagate, FIELDS_BODY, FIELDS_HEADER, FIELDS_PROOF,
};
use crate::stream::StreamSupport;
use crate::support::CoordinatorSupport;
use crate::verifier::{Verifier, VerifyError};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::FutureExt;
use futures_timer::Delay;
use tokio::time::Duration;

const PEER_KNOWN_BLOCKS_SIZE: u32 = 1024;
const PENDING_BLOCKS_SIZE: u32 = 2560;
const PEER_REQUEST_BLOCK_SIZE: u32 = 128;
const PEER_KNOWN_TXS_SIZE: u32 = 10240;
const TX_PROPAGATE_MAX_BLOCK_BEHIND: u32 = 8;
const BLOCK_REQUEST_TIMEOUT_S: u64 = 30;

pub struct ChainSync<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	peers: HashMap<PeerId, PeerInfo>,
	pending_blocks: BTreeMap<BlockNumber, PendingBlockInfo>,
	support: Arc<StreamSupport<S>>,
	verifier: Verifier<S>,
	next_request_id: RequestId,
	pub block_request_timer: FuturesUnordered<BoxFuture<'static, (PeerId, RequestId)>>,
}

/// Handle peers open/close
impl<S> ChainSync<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	pub fn new(support: Arc<StreamSupport<S>>) -> CommonResult<Self> {
		let verifier = Verifier::new(support.clone())?;

		Ok(ChainSync {
			peers: HashMap::new(),
			pending_blocks: BTreeMap::new(),
			support,
			verifier,
			next_request_id: RequestId(0),
			block_request_timer: FuturesUnordered::new(),
		})
	}

	pub fn on_protocol_open(&mut self, peer_id: PeerId) -> CommonResult<()> {
		self.peers.insert(
			peer_id.clone(),
			PeerInfo {
				known_blocks: LruCache::new(PEER_KNOWN_BLOCKS_SIZE as usize),
				known_txs: LruCache::new(PEER_KNOWN_TXS_SIZE as usize),
				confirmed_number: 0,
				confirmed_hash: self.support.get_genesis_hash().clone(),
				state: PeerState::Vacant,
			},
		);

		// announce block
		let (block_hash, header) = self
			.support
			.get_header_by_number(&self.support.get_confirmed_number()?)?;
		self.announce_block(block_hash, header, MessageTarget::One(peer_id))?;

		Ok(())
	}

	pub fn on_protocol_close(&mut self, peer_id: PeerId) -> CommonResult<()> {
		self.peers.remove(&peer_id);

		for pending_block_info in self.pending_blocks.values_mut() {
			match &pending_block_info.state {
				PendingBlockState::Downloading { from, .. } => {
					if **from == peer_id {
						pending_block_info.state = PendingBlockState::Seen;
					}
				}
				_ => (),
			}
		}
		self.sync()?;
		Ok(())
	}
}

/// Handle blocks syncing
impl<S> ChainSync<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	pub fn on_block_announce(
		&mut self,
		peer_id: PeerId,
		block_announce: BlockAnnounce,
	) -> CommonResult<()> {
		let BlockAnnounce { block_hash, header } = block_announce;
		let mut first_announce = false;
		if let Some(peer) = self.peers.get_mut(&peer_id) {
			first_announce = peer.confirmed_number == 0;
			peer.known_blocks.put(block_hash.clone(), ());
			peer.confirmed_number = header.number;
			peer.confirmed_hash = block_hash;
		}
		self.sync()?;
		if first_announce {
			self.on_block_announce_first(peer_id)?;
		}
		Ok(())
	}

	pub fn on_block_request(
		&mut self,
		peer_id: PeerId,
		block_request: BlockRequest,
	) -> CommonResult<()> {
		let mut blocks = vec![];
		let mut block_id = block_request.block_id;
		let fields = block_request.fields;
		let confirmed_number = self.support.get_confirmed_number()?;
		loop {
			let block_hash = match &block_id {
				BlockId::Number(number) => {
					Cow::Owned(self.support.get_block_hash_by_number(&number)?)
				}
				BlockId::Hash(hash) => Cow::Borrowed(hash),
			};
			let header = self.support.get_header_by_block_hash(block_hash.as_ref())?;
			let number = header.number;
			let header = if (fields & FIELDS_HEADER) == FIELDS_HEADER {
				Some(header)
			} else {
				None
			};

			let body = if (fields & FIELDS_BODY) == FIELDS_BODY {
				let body = self.support.get_body_by_block_hash(block_hash.as_ref())?;

				let meta_txs = body
					.meta_txs
					.into_iter()
					.map(|tx_hash| self.support.get_transaction_by_hash(&tx_hash))
					.collect::<CommonResult<Vec<_>>>()?;

				let payload_txs = body
					.payload_txs
					.into_iter()
					.map(|tx_hash| self.support.get_transaction_by_hash(&tx_hash))
					.collect::<CommonResult<Vec<_>>>()?;

				Some(BodyData {
					meta_txs,
					payload_txs,
				})
			} else {
				None
			};

			let proof = if (fields & FIELDS_PROOF) == FIELDS_PROOF {
				let proof = self.support.get_proof_by_block_hash(block_hash.as_ref())?;
				Some(proof)
			} else {
				None
			};

			let block_data = BlockData {
				number,
				block_hash: (*block_hash).clone(),
				header,
				body,
				proof,
			};
			blocks.push(block_data);

			if blocks.len() as u32 >= block_request.count {
				break;
			}

			block_id = match block_request.direction {
				Direction::Asc => {
					if number >= confirmed_number {
						break;
					}
					BlockId::Number(number + 1)
				}
				Direction::Desc => {
					if number == 0 {
						break;
					}
					BlockId::Number(number - 1)
				}
			}
		}

		let block_response = ProtocolMessage::BlockResponse(BlockResponse {
			request_id: block_request.request_id,
			blocks,
		});

		self.support
			.network_send_message(NetworkInMessage::SendMessage {
				peer_id,
				message: block_response.encode(),
			});

		Ok(())
	}

	pub fn on_block_request_timer_trigger(
		&mut self,
		peer_id: PeerId,
		request_id: RequestId,
	) -> CommonResult<()> {
		if let Some(peer_info) = self.peers.get_mut(&peer_id) {
			match &peer_info.state {
				PeerState::Downloading {
					request_id: rid,
					number,
					count,
				} if rid == &request_id => {
					trace!("Maintain downloading: block request timeout from {}, number: {}, count: {}, request_id: {}",
						   peer_id, number, count, request_id);
					peer_info.state = PeerState::Vacant;

					for pending_block_info in self.pending_blocks.values_mut() {
						match &pending_block_info.state {
							PendingBlockState::Downloading {
								request_id: rid, ..
							} if **rid == request_id => {
								pending_block_info.state = PendingBlockState::Seen;
							}
							_ => (),
						}
					}

					self.support
						.peer_manager_send_message(PMInMessage::ReportPeer(
							peer_id.clone(),
							PEER_REPORT_BLOCK_REQUEST_TIMEOUT,
						));

					self.sync()?;
				}
				_ => (),
			}
		}
		Ok(())
	}

	pub fn on_block_response(
		&mut self,
		peer_id: PeerId,
		block_response: BlockResponse,
	) -> CommonResult<()> {
		if let Some(peer_info) = self.peers.get_mut(&peer_id) {
			match &peer_info.state {
				PeerState::Downloading {
					request_id,
					number,
					count,
				} if request_id == &block_response.request_id => {
					trace!("Maintain downloading: receive block response from {}, number: {}, count: {}, request_id: {}",
							   peer_id, number, count, request_id);

					let from = Arc::new(peer_id);
					for block_data in block_response.blocks {
						let number = block_data.number;
						let pending_block_info = PendingBlockInfo {
							state: PendingBlockState::Downloaded {
								from: from.clone(),
								block_data: Some(block_data),
							},
						};
						self.pending_blocks.insert(number, pending_block_info);
					}
					peer_info.state = PeerState::Vacant;
				}
				_ => (),
			}
		}
		self.maintain_downloaded()?;

		Ok(())
	}

	pub fn on_block_committed(&mut self, _number: BlockNumber, hash: Hash) -> CommonResult<()> {
		// announce to all connected peers
		let (block_hash, header) = {
			let header = self.support.get_header_by_block_hash(&hash)?;
			let block_hash = hash;
			(block_hash, header)
		};
		self.announce_block(block_hash, header, MessageTarget::All)?;
		Ok(())
	}

	pub fn on_execution_committed(
		&mut self,
		_number: BlockNumber,
		_hash: Hash,
	) -> CommonResult<()> {
		self.maintain_downloaded()?;
		Ok(())
	}

	pub fn sync(&mut self) -> CommonResult<()> {
		self.maintain_new()?;
		self.maintain_seen()?;
		Ok(())
	}

	fn maintain_new(&mut self) -> CommonResult<()> {
		let max_peer_number = match self.peers.values().map(|x| x.confirmed_number).max() {
			Some(v) => v,
			None => return Ok(()),
		};
		let max_pending_number = match self.pending_blocks.iter().last() {
			Some((k, _v)) => *k,
			None => self.support.get_confirmed_number()?,
		};
		let max_to_append = u64::min(
			max_peer_number,
			max_pending_number + PENDING_BLOCKS_SIZE as u64 - self.pending_blocks.len() as u64,
		);
		let min_to_append = max_pending_number + 1;
		if max_to_append >= min_to_append {
			for number in min_to_append..=max_to_append {
				self.pending_blocks.insert(
					number,
					PendingBlockInfo {
						state: PendingBlockState::Seen,
					},
				);
			}
			trace!(
				"Maintain new: insert pending blocks ({} to {})",
				min_to_append,
				max_to_append
			);
		}
		Ok(())
	}

	fn maintain_seen(&mut self) -> CommonResult<()> {
		let seen_groups = make_seen_groups(&self.pending_blocks, PEER_REQUEST_BLOCK_SIZE);
		for (number, size) in seen_groups {
			// find the peer with the min confirm number, that is vacant and has the required blocks
			let peer = self
				.peers
				.iter_mut()
				.filter_map(|(peer_id, peer_info)| {
					let vacant = matches!(peer_info.state, PeerState::Vacant);
					let has_blocks = peer_info.confirmed_number >= number + size as u64 - 1;
					if vacant && has_blocks {
						Some((peer_id, peer_info))
					} else {
						None
					}
				})
				.min_by(|a, b| Ord::cmp(&a.1.confirmed_number, &b.1.confirmed_number));

			if let Some((peer_id, peer_info)) = peer {
				let request_id = Self::next_request_id(&mut self.next_request_id);
				let block_request = ProtocolMessage::BlockRequest(BlockRequest {
					request_id: request_id.clone(),
					fields: FIELDS_HEADER | FIELDS_BODY | FIELDS_PROOF,
					block_id: BlockId::Number(number),
					count: size,
					direction: Direction::Asc,
				});
				self.support
					.network_send_message(NetworkInMessage::SendMessage {
						peer_id: peer_id.clone(),
						message: block_request.encode(),
					});
				peer_info.state = PeerState::Downloading {
					request_id: request_id.clone(),
					number,
					count: size,
				};
				let from = Arc::new(peer_id.clone());
				let request_id = Arc::new(request_id);
				for n in number..number + (size as u64) {
					if let Some(v) = self.pending_blocks.get_mut(&n) {
						v.state = PendingBlockState::Downloading {
							from: from.clone(),
							request_id: request_id.clone(),
						}
					}
				}
				// setup timer
				let timer_result = (peer_id.clone(), (*request_id).clone());
				self.block_request_timer.push(
					async {
						Delay::new(Duration::from_secs(BLOCK_REQUEST_TIMEOUT_S)).await;
						timer_result
					}
					.boxed(),
				);

				trace!(
					"Maintain seen: send block request to {}, number: {}, count: {}",
					peer_id,
					number,
					size
				);
			}
		}
		Ok(())
	}

	fn maintain_downloaded(&mut self) -> CommonResult<()> {
		loop {
			let first_downloaded_number = {
				let (number, pending_block) = match self.pending_blocks.iter().next() {
					Some(v) => v,
					None => break,
				};
				match &pending_block.state {
					PendingBlockState::Downloaded { .. } => (),
					_ => break,
				};
				*number
			};

			// take the first pending_block
			let mut pending_block = match self.pending_blocks.remove(&first_downloaded_number) {
				Some(v) => v,
				None => break,
			};
			// take block_data from pending_block
			let mut block_data = match &mut pending_block.state {
				PendingBlockState::Downloaded { block_data, .. } => block_data.take(),
				_ => unreachable!("qed"),
			};

			let result = self.verifier.verify_block(&mut block_data);
			let result_desc = match &result {
				Ok(_v) => "Ok".to_string(),
				Err(e) => e.to_string(),
			};
			let action = self.on_verify_result(result)?;

			trace!(
				"Maintain downloaded: block: {}, result: {}, action: {:?}",
				first_downloaded_number,
				result_desc,
				action,
			);

			match action {
				VerifyAction::Ok => (),
				VerifyAction::Wait => {
					// put block_data back to pending_block
					match &mut pending_block.state {
						PendingBlockState::Downloaded { block_data: bd, .. } => *bd = block_data,
						_ => unreachable!("qed"),
					};
					// put pending_block back
					self.pending_blocks
						.insert(first_downloaded_number, pending_block);
					break;
				}
				VerifyAction::Discard => {
					break;
				}
				VerifyAction::Reset => {
					let peer_id = match pending_block.state {
						PendingBlockState::Downloaded { from, .. } => from,
						_ => unreachable!("qed"),
					};
					self.pending_blocks
						.iter_mut()
						.for_each(|(_number, pending_block)| match &pending_block.state {
							PendingBlockState::Downloaded { from, .. } => {
								if from == &peer_id {
									pending_block.state = PendingBlockState::Seen;
								}
							}
							_ => (),
						});
					self.support
						.peer_manager_send_message(PMInMessage::ReportPeer(
							(*peer_id).clone(),
							PEER_REPORT_INVALID_BLOCK,
						));
					break;
				}
			}
		}
		self.sync()?;

		Ok(())
	}

	fn on_block_announce_first(&mut self, peer_id: PeerId) -> CommonResult<()> {
		if let Some(peer) = self.peers.get_mut(&peer_id) {
			let current_number = self
				.support
				.ori_support()
				.get_current_state()
				.confirmed_number;
			if Self::should_tx_propagate(peer.confirmed_number, current_number) {
				let txs = self.support.ori_support().txpool_get_transactions()?;
				self.propagate_txs(txs, MessageTarget::One(peer_id))?;
			}
		}

		Ok(())
	}

	fn on_verify_ok(
		&self,
		commit_block_params: ChainCommitBlockParams,
	) -> CommonResult<VerifyAction> {
		let tx_hash_set = commit_block_params
			.body
			.meta_txs
			.iter()
			.chain(commit_block_params.body.payload_txs.iter())
			.cloned()
			.collect::<HashSet<_>>();
		let mut remove_from_txpool = false;

		let action = self
			.support
			.ori_support()
			.commit_block(commit_block_params)
			.map(|_| {
				remove_from_txpool = true;
				VerifyAction::Ok
			})
			.or_else_catch::<node_chain::errors::ErrorKind, _>(|e| match e {
				node_chain::errors::ErrorKind::CommitBlockError(e) => {
					let action = match e {
						node_chain::errors::CommitBlockError::Duplicated => {
							remove_from_txpool = true;
							VerifyAction::Discard
						}
						node_chain::errors::CommitBlockError::NotBest => VerifyAction::Reset,
					};
					Some(Ok(action))
				}
				_ => None,
			})?;

		if remove_from_txpool {
			trace!(
				"Remove txs from txpool after committing block, count: {}",
				tx_hash_set.len()
			);
			self.support
				.ori_support()
				.txpool_remove_transactions(&tx_hash_set)?;
		}

		Ok(action)
	}

	fn on_verify_err(&self, e: &VerifyError) -> CommonResult<VerifyAction> {
		let action = match e {
			VerifyError::ShouldWait => VerifyAction::Wait,
			VerifyError::Duplicated => VerifyAction::Discard,
			VerifyError::NotBest => VerifyAction::Reset,
			VerifyError::Bad => VerifyAction::Reset,
			VerifyError::InvalidExecutionGap => VerifyAction::Reset,
			VerifyError::InvalidHeader(_) => VerifyAction::Reset,
			VerifyError::DuplicatedTx(_) => VerifyAction::Reset,
			VerifyError::InvalidTx(_) => VerifyAction::Reset,
			VerifyError::InvalidProof(_) => VerifyAction::Reset,
		};
		Ok(action)
	}

	fn on_verify_result(
		&self,
		result: CommonResult<ChainCommitBlockParams>,
	) -> CommonResult<VerifyAction> {
		let action = result
			.and_then(|v| self.on_verify_ok(v))
			.or_else_catch::<ErrorKind, _>(|e| match e {
				ErrorKind::VerifyError(e) => Some(self.on_verify_err(e)),
				_ => None,
			})?;
		Ok(action)
	}

	fn announce_block(
		&mut self,
		block_hash: Hash,
		header: Header,
		target: MessageTarget,
	) -> CommonResult<()> {
		let number = header.number;
		let message = ProtocolMessage::BlockAnnounce(BlockAnnounce {
			block_hash: block_hash.clone(),
			header,
		});
		let message = message.encode();

		let peers = self.peers.iter_mut().filter(|&(peer_id, _)| match &target {
			MessageTarget::All => true,
			MessageTarget::One(target_peer_id) => peer_id == target_peer_id,
		});

		for (peer_id, peer_info) in peers {
			if peer_info.known_blocks.put(block_hash.clone(), ()).is_none() {
				trace!(
					"Announce block to {}, block_hash: {}, number: {}",
					peer_id,
					block_hash,
					number,
				);

				self.support
					.network_send_message(NetworkInMessage::SendMessage {
						peer_id: peer_id.clone(),
						message: message.clone(),
					});
			}
		}
		Ok(())
	}

	fn next_request_id(request_id: &mut RequestId) -> RequestId {
		let new = RequestId(request_id.0.checked_add(1).unwrap_or(0));
		std::mem::replace(request_id, new)
	}

	fn should_tx_propagate(receiver_number: BlockNumber, sender_number: BlockNumber) -> bool {
		receiver_number + TX_PROPAGATE_MAX_BLOCK_BEHIND as u64 >= sender_number
	}
}

/// Handle tx propagation
impl<S> ChainSync<S>
where
	S: CoordinatorSupport + Send + Sync + 'static,
{
	pub fn on_tx_inserted(&mut self, tx_hash: Hash) -> CommonResult<()> {
		if let Some(tx) = self
			.support
			.ori_support()
			.txpool_get_transaction(&tx_hash)?
		{
			self.propagate_txs(vec![tx], MessageTarget::All)?;
		}
		Ok(())
	}

	pub fn on_tx_propagate(
		&mut self,
		peer_id: PeerId,
		tx_propagate: TxPropagate,
	) -> CommonResult<()> {
		let ori_support = self.support.ori_support();
		let current_number = ori_support.get_current_state().confirmed_number;
		let peer_max_number = self
			.peers
			.values()
			.map(|x| x.confirmed_number)
			.max()
			.unwrap_or(0);
		if !Self::should_tx_propagate(current_number, peer_max_number) {
			return Ok(());
		}

		for tx in tx_propagate.txs {
			if let Some(peer_info) = self.peers.get_mut(&peer_id) {
				let tx_hash = self.support.ori_support().hash_transaction(&tx)?;
				if peer_info.known_txs.put(tx_hash, ()).is_none() {
					let result = ori_support.txpool_insert_transaction(tx);
					self.on_insert_result(result, &peer_id)?;
				}
			}
		}
		Ok(())
	}

	fn on_insert_result(&self, result: CommonResult<()>, peer_id: &PeerId) -> CommonResult<()> {
		result.or_else_catch::<node_txpool::errors::ErrorKind, _>(|e| match e {
			node_txpool::errors::ErrorKind::InsertError(e) => {
				info!("Insert into txpool error: {}, source: {}", e, peer_id);
				match e {
					node_txpool::errors::InsertError::InvalidTx(_e) => {
						self.support
							.peer_manager_send_message(PMInMessage::ReportPeer(
								peer_id.clone(),
								PEER_REPORT_INVALID_TX,
							));
					}
					_ => (),
				}
				Some(Ok(()))
			}
			_ => None,
		})?;
		Ok(())
	}

	fn propagate_txs(
		&mut self,
		txs: Vec<Arc<FullTransaction>>,
		target: MessageTarget,
	) -> CommonResult<()> {
		let peers = self.peers.iter_mut().filter(|&(peer_id, _)| match &target {
			MessageTarget::All => true,
			MessageTarget::One(target_peer_id) => peer_id == target_peer_id,
		});

		for (peer_id, peer_info) in peers {
			let to_propagate_txs = txs
				.iter()
				.filter(|tx| peer_info.known_txs.put(tx.tx_hash.clone(), ()).is_none())
				.map(|tx| tx.tx.clone())
				.collect::<Vec<_>>();
			if !to_propagate_txs.is_empty() {
				trace!(
					"Propagate txs to {}, count: {}",
					peer_id,
					to_propagate_txs.len(),
				);
				let tx_propagate = ProtocolMessage::TxPropagate(TxPropagate {
					txs: to_propagate_txs,
				});
				self.support
					.network_send_message(NetworkInMessage::SendMessage {
						peer_id: peer_id.clone(),
						message: tx_propagate.encode(),
					});
			}
		}
		Ok(())
	}
}

enum MessageTarget {
	All,
	One(PeerId),
}

#[derive(Debug)]
enum VerifyAction {
	Ok,
	Wait,
	Discard,
	Reset,
}

#[derive(Debug)]
pub struct PeerInfo {
	known_blocks: LruCache<Hash, ()>,
	known_txs: LruCache<Hash, ()>,
	confirmed_number: BlockNumber,
	confirmed_hash: Hash,
	state: PeerState,
}

#[derive(Debug)]
pub struct PendingBlockInfo {
	state: PendingBlockState,
}

#[derive(Debug)]
pub enum PeerState {
	Vacant,
	Downloading {
		request_id: RequestId,
		number: BlockNumber,
		count: u32,
	},
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum PendingBlockState {
	Seen,
	Downloading {
		from: Arc<PeerId>,
		request_id: Arc<RequestId>,
	},
	Downloaded {
		from: Arc<PeerId>,
		block_data: Option<BlockData>,
	},
}

fn make_seen_groups(
	pending_blocks: &BTreeMap<BlockNumber, PendingBlockInfo>,
	max_group_size: u32,
) -> Vec<(BlockNumber, u32)> {
	let mut groups = vec![];
	let mut item: Option<(BlockNumber, u32)> = None;
	for (number, pending_block_info) in pending_blocks {
		match pending_block_info.state {
			PendingBlockState::Seen => match item {
				None => {
					item = Some((*number, 1));
				}
				Some(v) => {
					item = Some((v.0, v.1 + 1));
				}
			},
			_ => match item {
				Some(v) => {
					groups.push(v);
					item = None;
				}
				None => (),
			},
		}

		if let Some(v) = item {
			if v.1 >= max_group_size {
				groups.push(v);
				item = None;
			}
		}
	}
	if let Some(item) = item {
		groups.push(item);
	}
	groups
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_make_seen_groups() {
		let peer_id = Arc::new(PeerId::random());

		let pending_blocks = vec![
			(
				1u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				2u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				3u64,
				PendingBlockInfo {
					state: PendingBlockState::Downloading {
						from: peer_id.clone(),
						request_id: Arc::new(RequestId(0)),
					},
				},
			),
			(
				4u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				5u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				6u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				7u64,
				PendingBlockInfo {
					state: PendingBlockState::Downloading {
						from: peer_id.clone(),
						request_id: Arc::new(RequestId(0)),
					},
				},
			),
			(
				8u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				9u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
		]
		.into_iter()
		.collect::<BTreeMap<_, _>>();

		let groups = make_seen_groups(&pending_blocks, 2);

		println!("groups: {:?}", groups);
	}

	#[test]
	fn test_make_seen_groups2() {
		let peer_id = Arc::new(PeerId::random());

		let pending_blocks = vec![
			(
				1u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				2u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				3u64,
				PendingBlockInfo {
					state: PendingBlockState::Downloading {
						from: peer_id.clone(),
						request_id: Arc::new(RequestId(0)),
					},
				},
			),
			(
				4u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				5u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				6u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				7u64,
				PendingBlockInfo {
					state: PendingBlockState::Downloading {
						from: peer_id.clone(),
						request_id: Arc::new(RequestId(0)),
					},
				},
			),
			(
				8u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				9u64,
				PendingBlockInfo {
					state: PendingBlockState::Seen,
				},
			),
			(
				10u64,
				PendingBlockInfo {
					state: PendingBlockState::Downloading {
						from: peer_id.clone(),
						request_id: Arc::new(RequestId(0)),
					},
				},
			),
		]
		.into_iter()
		.collect::<BTreeMap<_, _>>();

		let groups = make_seen_groups(&pending_blocks, 2);

		println!("groups: {:?}", groups);
	}
}
