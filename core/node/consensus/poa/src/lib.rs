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
use log::{debug, info, warn};

use crypto::address::Address as AddressT;
use crypto::dsa::{Dsa, KeyPair};
use node_consensus::errors::ErrorKind;
use node_consensus::{errors, support::ConsensusSupport};
use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::errors::CommonResult;
use primitives::types::ExecutionGap;
use primitives::{Address, BlockNumber, BuildBlockParams, FullTransaction, SecretKey};

pub struct PoaConfig {
	pub secret_key: SecretKey,
}

pub struct Poa<S>
where
	S: ConsensusSupport,
{
	stream: Arc<PoaStream<S>>,
	is_authority: bool,
}

impl<S> Poa<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	pub fn new(config: PoaConfig, support: Arc<S>) -> CommonResult<Self> {
		let system_meta = get_system_meta(&support)?;
		let poa_meta = get_poa_meta(&support)?;
		let authority_address = get_poa_authority(&support)?;
		let current_address = get_current_address(&config.secret_key, &support)?;
		let is_authority = current_address == authority_address;

		info!(
			"Current node is authority: {}, authority address: {}, current address: {}",
			is_authority, authority_address, current_address
		);

		let stream = Arc::new(PoaStream {
			system_meta,
			poa_meta,
			support,
		});

		if is_authority && stream.poa_meta.block_interval.is_some() {
			tokio::spawn(start(stream.clone()));
		}

		info!("Initializing consensus poa");

		let poa = Poa {
			stream,
			is_authority,
		};

		Ok(poa)
	}

	pub async fn generate_block(&self) -> CommonResult<()> {
		if !self.is_authority {
			return Err(errors::ErrorKind::Other("Not authority".to_string()).into());
		}

		let timestamp = SystemTime::now();
		let timestamp = timestamp
			.duration_since(SystemTime::UNIX_EPOCH)
			.map_err(|_| ErrorKind::TimeError)?;
		let timestamp = timestamp.as_millis() as u64;
		let schedule_info = ScheduleInfo { timestamp };
		self.stream.work(schedule_info).await?;
		Ok(())
	}
}

struct PoaStream<S>
where
	S: ConsensusSupport,
{
	system_meta: module::system::Meta,
	poa_meta: module::poa::Meta,
	support: Arc<S>,
}

impl<S> PoaStream<S>
where
	S: ConsensusSupport,
{
	async fn work(&self, schedule_info: ScheduleInfo) -> CommonResult<()> {
		let confirmed_number = match self.support.get_confirmed_number() {
			Ok(number) => number.expect("qed"),
			Err(e) => {
				warn!("Unable to get best number: {}", e);
				return Ok(());
			}
		};

		let number = confirmed_number + 1;
		let timestamp = schedule_info.timestamp;

		let txs = match self.support.get_transactions_in_txpool() {
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
			if self.validate_transaction(tx, number).is_err() {
				invalid_txs.push(tx.clone());
				continue;
			}
			let is_meta = self.support.is_meta_tx(&*(&tx.tx)).expect("qed");
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

		let execution_number =
			self.support
				.get_execution_number()?
				.ok_or(errors::ErrorKind::Data(format!(
					"Execution number not found"
				)))?;

		let block_execution_gap = (number - execution_number) as ExecutionGap;
		if block_execution_gap > self.system_meta.max_execution_gap {
			warn!(
				"execution gap exceed max: {}, max: {}",
				block_execution_gap, self.system_meta.max_execution_gap
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

		let result = self.support.commit_block(commit_block_params)?;

		match result {
			Ok(_) => {
				let tx_hash_set = txs
					.iter()
					.map(|x| x.tx_hash.clone())
					.collect::<HashSet<_>>();

				self.support.remove_transactions_in_txpool(&tx_hash_set)?;
			}
			Err(e) => {
				warn!("Commit block failed: {:?}", e);
			}
		}

		Ok(())
	}

	fn validate_transaction(
		&self,
		tx: &Arc<FullTransaction>,
		number: BlockNumber,
	) -> CommonResult<()> {
		if self.support.get_transaction(&tx.tx_hash)?.is_some() {
			return Err(errors::ErrorKind::Duplicated(tx.tx_hash.clone()).into());
		}
		let witness = tx.tx.witness.as_ref().expect("qed");

		if witness.until < number {
			return Err(errors::ErrorKind::InvalidUntil(tx.tx_hash.clone()).into());
		}

		Ok(())
	}
}

async fn start<S>(stream: Arc<PoaStream<S>>) -> CommonResult<()>
where
	S: ConsensusSupport,
{
	info!("Start poa work");
	let block_interval = stream.poa_meta.block_interval.expect("qed");
	let mut scheduler = Scheduler::new(block_interval);
	loop {
		let item = scheduler.next().await;
		match item {
			Some(Ok(v)) => match stream.work(v).await {
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

fn get_system_meta<S: ConsensusSupport>(support: &Arc<S>) -> CommonResult<module::system::Meta> {
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

fn get_poa_meta<S: ConsensusSupport>(support: &Arc<S>) -> CommonResult<module::poa::Meta> {
	support
		.execute_call_with_block_number(
			&0,
			None,
			"poa".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}

fn get_poa_authority<S: ConsensusSupport>(support: &Arc<S>) -> CommonResult<Address> {
	support
		.execute_call_with_block_number(
			&0,
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
