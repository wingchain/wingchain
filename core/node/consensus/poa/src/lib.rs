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

//! POA consensus
//! one node append new blocks at a certain frequency specified by block interval

use std::collections::HashSet;
use std::convert::TryInto;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::time::{Duration, SystemTime};

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::prelude::*;
use futures::task::Poll;
use futures::{Future, Stream};
use futures_timer::Delay;
use log::{debug, error, info, trace, warn};
use parking_lot::RwLock;

use crypto::address::Address as AddressT;
use crypto::dsa::{Dsa, KeyPair};
use node_consensus_base::{
	support::ConsensusSupport, Consensus as ConsensusT, ConsensusConfig, ConsensusInMessage,
	ConsensusOutMessage,
};
use node_consensus_primitives::CONSENSUS_POA;
use node_executor::module;
use node_executor::module::poa::Meta;
use node_executor_primitives::EmptyParams;
use primitives::errors::{Catchable, CommonResult};
use primitives::types::ExecutionGap;
use primitives::{
	codec, Address, BlockNumber, BuildBlockParams, FullTransaction, Header, SecretKey,
};

use crate::proof::Proof;

pub mod proof;

pub struct Poa<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
	in_tx: UnboundedSender<ConsensusInMessage>,
	out_rx: RwLock<Option<UnboundedReceiver<ConsensusOutMessage>>>,
}

impl<S> Poa<S>
where
	S: ConsensusSupport,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let poa_meta = get_poa_meta(&support, &0)?;

		let current_secret_key = config.secret_key;
		let current_address = if let Some(secret_key) = &current_secret_key {
			Some(get_address(secret_key, &support)?)
		} else {
			None
		};
		let number = support.get_current_state().confirmed_number;
		let authority_address = get_poa_authority(&support, &number)?;
		let is_authority = current_address.as_ref() == Some(&authority_address);

		info!(
			"Current node is authority: {}, authority address: {}, current address: {:?}",
			is_authority, authority_address, current_address
		);

		let (in_tx, in_rx) = unbounded();
		let (out_tx, out_rx) = unbounded();

		PoaStream::spawn(
			support.clone(),
			poa_meta,
			current_secret_key,
			current_address,
			out_tx,
			in_rx,
		);

		info!("Initializing consensus poa");

		let poa = Poa {
			support,
			in_tx,
			out_rx: RwLock::new(Some(out_rx)),
		};

		Ok(poa)
	}
}

impl<S> ConsensusT for Poa<S>
where
	S: ConsensusSupport,
{
	fn verify_proof(&self, header: &Header, proof: &primitives::Proof) -> CommonResult<()> {
		let name = &proof.name;
		if name != CONSENSUS_POA {
			return Err(
				node_consensus_base::errors::ErrorKind::VerifyProofError(format!(
					"Unexpected consensus: {}",
					name
				))
				.into(),
			);
		}
		let data = &proof.data;
		let proof: Proof = codec::decode(data).map_err(|_| {
			node_consensus_base::errors::ErrorKind::VerifyProofError("Decode error".to_string())
		})?;

		let address = {
			let addresser = self.support.get_basic()?.address.clone();
			let address_len = addresser.length().into();
			let mut address = vec![0u8; address_len];
			addresser.address(&mut address, &proof.public_key.0);
			Address(address)
		};

		let authority_address = get_poa_authority(&self.support, &(header.number - 1))?;
		let is_authority = address == authority_address;
		if !is_authority {
			return Err(node_consensus_base::errors::ErrorKind::VerifyProofError(
				"Not authority".to_string(),
			)
			.into());
		}
		Ok(())
	}

	fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		self.in_tx.clone()
	}

	fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		self.out_rx.write().take()
	}
}

struct PoaStream<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
	poa_meta: Meta,
	current_secret_key: Option<SecretKey>,
	current_address: Option<Address>,
	#[allow(dead_code)]
	out_tx: UnboundedSender<ConsensusOutMessage>,
	in_rx: UnboundedReceiver<ConsensusInMessage>,
}

impl<S> PoaStream<S>
where
	S: ConsensusSupport,
{
	fn spawn(
		support: Arc<S>,
		poa_meta: Meta,
		current_secret_key: Option<SecretKey>,
		current_address: Option<Address>,
		out_tx: UnboundedSender<ConsensusOutMessage>,
		in_rx: UnboundedReceiver<ConsensusInMessage>,
	) {
		let this = Self {
			support,
			poa_meta,
			current_secret_key,
			current_address,
			out_tx,
			in_rx,
		};
		if this.current_address.is_some() {
			tokio::spawn(this.start());
		}
	}

	async fn start(mut self) {
		info!("Start poa work");
		let mut scheduler = Scheduler::new(self.poa_meta.block_interval);
		loop {
			tokio::select! {
				Some(item) = scheduler.next() => {
					match item {
						Ok(v) => {
							self.work(v)
								.unwrap_or_else(|e| error!("Consensus poa handle work error: {}", e));
						},
						Err(e) => {
							error!("Terminated with an error: {:?}", e);
							break;
						}
					}
				}
				Some(in_message) = self.in_rx.next() => {
					self.on_in_message(in_message)
						.unwrap_or_else(|e| error!("Consensus poa handle in message error: {}", e));
				}
			}
		}
	}

	fn work(&self, schedule_info: ScheduleInfo) -> CommonResult<()> {
		let current_state = &self.support.get_current_state();

		let authority_address = get_poa_authority(&self.support, &current_state.confirmed_number)?;
		let is_authority = self.current_address.as_ref() == Some(&authority_address);

		trace!(
			"Current node is authority: {}, authority address: {}, current address: {:?}",
			is_authority,
			authority_address,
			self.current_address
		);

		if !is_authority {
			return Ok(());
		}

		let system_meta = &current_state.system_meta;
		let number = current_state.confirmed_number + 1;
		let timestamp = schedule_info.timestamp;
		let execution_number = current_state.executed_number;

		let txs = match self.support.txpool_get_transactions() {
			Ok(txs) => txs,
			Err(e) => {
				warn!("Unable to get transactions in txpool: {}", e);
				return Ok(());
			}
		};

		debug!("TxPool txs count: {}", txs.len());

		let mut invalid_txs = vec![];
		let mut meta_txs = vec![];
		let mut payload_txs = vec![];

		for tx in &txs {
			let invalid = self
				.validate_transaction(tx)
				.map(|_| false)
				.or_else_catch::<node_chain::errors::ErrorKind, _>(|e| match e {
				node_chain::errors::ErrorKind::ValidateTxError(_e) => Some(Ok(true)),
				_ => None,
			})?;

			if invalid {
				invalid_txs.push(tx.clone());
				continue;
			}
			let is_meta = self.support.is_meta_tx(&tx.tx).expect("qed");
			match is_meta {
				true => {
					meta_txs.push(tx.clone());
				}
				false => {
					payload_txs.push(tx.clone());
				}
			}
		}
		debug!("Invalid txs count: {}", invalid_txs.len());

		let block_execution_gap = (number - execution_number) as ExecutionGap;
		if block_execution_gap > system_meta.max_execution_gap {
			warn!(
				"execution gap exceed max: {}, max: {}",
				block_execution_gap, system_meta.max_execution_gap
			);
			return Ok(());
		}

		let build_block_params = BuildBlockParams {
			number,
			timestamp,
			meta_txs,
			payload_txs,
			execution_number,
		};

		let mut commit_block_params = self.support.build_block(build_block_params)?;

		let current_secret_key = self.current_secret_key.as_ref().expect("qed");
		let proof = Proof::new(
			&commit_block_params.block_hash,
			current_secret_key,
			self.support.get_basic()?.dsa.clone(),
		)?;
		commit_block_params.proof = proof.try_into()?;

		self.support
			.commit_block(commit_block_params)
			.or_else_catch::<node_chain::errors::ErrorKind, _>(|e| match e {
				node_chain::errors::ErrorKind::CommitBlockError(e) => {
					warn!("Commit block error: {:?}", e);
					Some(Ok(()))
				}
				_ => None,
			})?;

		let tx_hash_set = txs
			.iter()
			.map(|x| x.tx_hash.clone())
			.collect::<HashSet<_>>();

		self.support.txpool_remove_transactions(&tx_hash_set)?;

		Ok(())
	}

	fn validate_transaction(&self, tx: &Arc<FullTransaction>) -> CommonResult<()> {
		self.support
			.validate_transaction(&tx.tx_hash, &tx.tx, true)?;
		Ok(())
	}

	fn generate(&self) -> CommonResult<()> {
		let timestamp = SystemTime::now();
		let timestamp = timestamp
			.duration_since(SystemTime::UNIX_EPOCH)
			.map_err(|_| node_consensus_base::errors::ErrorKind::Time)?;
		let timestamp = timestamp.as_millis() as u64;
		let schedule_info = ScheduleInfo { timestamp };
		self.work(schedule_info)?;
		Ok(())
	}

	#[allow(clippy::single_match)]
	fn on_in_message(&self, in_message: ConsensusInMessage) -> CommonResult<()> {
		match in_message {
			ConsensusInMessage::Generate => {
				println!("generate");
				self.generate()?;
			}
			_ => {}
		}
		Ok(())
	}
}

fn get_poa_meta<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<module::poa::Meta> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"poa".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}

fn get_poa_authority<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<Address> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"poa".to_string(),
			"get_authority".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}

fn get_address<S: ConsensusSupport>(
	secret_key: &SecretKey,
	support: &Arc<S>,
) -> CommonResult<Address> {
	let dsa = support.get_basic()?.dsa.clone();
	let (_, public_key_len, _) = dsa.length().into();
	let mut public_key = vec![0u8; public_key_len];
	dsa.key_pair_from_secret_key(&secret_key.0)?
		.public_key(&mut public_key);

	let addresser = support.get_basic()?.address.clone();
	let address_len = addresser.length().into();
	let mut address = vec![0u8; address_len];
	addresser.address(&mut address, &public_key);

	let address = Address(address);
	Ok(address)
}

struct Scheduler {
	duration: Option<u64>,
	delay: Option<Delay>,
}

impl Scheduler {
	fn new(duration: Option<u64>) -> Self {
		Self {
			duration,
			delay: None,
		}
	}
}

struct ScheduleInfo {
	timestamp: u64,
}

impl Stream for Scheduler {
	type Item = Result<ScheduleInfo, ()>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let duration = match self.duration {
			Some(v) => v,
			None => return Poll::Pending,
		};

		self.delay = match self.delay.take() {
			None => {
				// schedule wait.
				let wait_duration = time_until_next(duration_now(), duration);
				Some(Delay::new(wait_duration))
			}
			Some(d) => Some(d),
		};

		if let Some(ref mut delay) = self.delay {
			match Future::poll(Pin::new(delay), cx) {
				Poll::Pending => return Poll::Pending,
				Poll::Ready(()) => {}
			}
		}

		self.delay = None;

		let timestamp = SystemTime::now();
		let timestamp = match timestamp.duration_since(SystemTime::UNIX_EPOCH) {
			Ok(timestamp) => timestamp.as_millis() as u64,
			Err(_) => return Poll::Ready(Some(Err(()))),
		};

		Poll::Ready(Some(Ok(ScheduleInfo { timestamp })))
	}
}

fn time_until_next(now: Duration, duration: u64) -> Duration {
	let remaining_full_millis = duration - (now.as_millis() as u64 % duration) - 1;
	Duration::from_millis(remaining_full_millis)
}

fn duration_now() -> Duration {
	let now = SystemTime::now();
	now.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap_or_else(|e| {
			panic!(
				"Current time {:?} is before unix epoch. Something is wrong: {:?}",
				now, e,
			)
		})
}
