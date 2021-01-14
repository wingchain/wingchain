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
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::time::{Duration, SystemTime};

use futures::prelude::*;
use futures::task::Poll;
use futures::{Future, Stream};
use futures_timer::Delay;
use log::{debug, info, trace, warn};

use crypto::address::Address as AddressT;
use crypto::dsa::{Dsa, KeyPair};
use node_consensus_base::{support::ConsensusSupport, Consensus as ConsensusT, ConsensusConfig};
use node_executor::module;
use node_executor::module::poa::Meta;
use node_executor_primitives::EmptyParams;
use primitives::errors::{Catchable, CommonResult};
use primitives::types::ExecutionGap;
use primitives::{Address, BlockNumber, BuildBlockParams, FullTransaction, SecretKey};

pub mod errors;

pub struct Poa<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	stream: Arc<PoaStream<S>>,
}

impl<S> Poa<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let poa_meta = get_poa_meta(&support, &0)?;

		let current_address = if let Some(secret_key) = &config.secret_key {
			Some(get_current_address(secret_key, &support)?)
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

		let stream = Arc::new(PoaStream {
			support,
			poa_meta,
			current_address,
		});

		tokio::spawn(start(stream.clone()));

		info!("Initializing consensus poa");

		let poa = Poa { stream };

		Ok(poa)
	}
}

impl<S> ConsensusT for Poa<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	fn generate(&self) -> CommonResult<()> {
		let timestamp = SystemTime::now();
		let timestamp = timestamp
			.duration_since(SystemTime::UNIX_EPOCH)
			.map_err(|_| node_consensus_base::errors::ErrorKind::Time)?;
		let timestamp = timestamp.as_millis() as u64;
		let schedule_info = ScheduleInfo { timestamp };
		self.stream.work(schedule_info)?;
		Ok(())
	}
}

struct PoaStream<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	support: Arc<S>,
	poa_meta: Meta,
	current_address: Option<Address>,
}

impl<S> PoaStream<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
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

		let commit_block_params = self.support.build_block(build_block_params)?;

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
}

async fn start<S>(stream: Arc<PoaStream<S>>) -> CommonResult<()>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	let block_interval = match stream.poa_meta.block_interval {
		Some(v) => v,
		None => return Ok(()),
	};
	info!("Start poa work");
	let mut scheduler = Scheduler::new(block_interval);
	loop {
		let item = scheduler.next().await;
		match item {
			Some(Ok(v)) => match stream.work(v) {
				Ok(_) => (),
				Err(e) => warn!("Encountered consensus error: {:?}", e),
			},
			Some(Err(e)) => {
				warn!("Terminated with an error: {:?}", e);
				break;
			}
			None => break,
		}
	}

	Ok(())
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

fn get_current_address<S: ConsensusSupport>(
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
	duration: u64,
	delay: Option<Delay>,
}

impl Scheduler {
	fn new(duration: u64) -> Self {
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
		self.delay = match self.delay.take() {
			None => {
				// schedule wait.
				let wait_duration = time_until_next(duration_now(), self.duration);
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
