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
use futures::{Future, Stream, TryStreamExt};
use futures_timer::Delay;
use log::{debug, info, warn};

use node_consensus::errors::ErrorKind;
use node_consensus::{errors, support::ConsensusSupport};
use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::errors::CommonResult;
use primitives::{BuildBlockParams, FullTransaction};

pub struct Poa<S>
where
	S: ConsensusSupport,
{
	#[allow(dead_code)]
	support: Arc<S>,
}

impl<S> Poa<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	pub fn new(support: Arc<S>) -> CommonResult<Self> {
		let poa = Poa {
			support: support.clone(),
		};

		let meta = get_poa_meta(support.clone())?;

		if meta.block_interval.is_some() {
			tokio::spawn(start(meta, support.clone()));
		}

		info!("Initializing consensus poa");

		Ok(poa)
	}

	pub async fn generate_block(&self) -> CommonResult<()> {
		let timestamp = SystemTime::now();
		let timestamp = timestamp
			.duration_since(SystemTime::UNIX_EPOCH)
			.map_err(|_| ErrorKind::TimeError)?;
		let timestamp = timestamp.as_millis() as u64;
		let schedule_info = ScheduleInfo { timestamp };
		work(schedule_info, self.support.clone()).await?;
		Ok(())
	}
}

async fn start<S>(meta: module::poa::Meta, support: Arc<S>) -> CommonResult<()>
where
	S: ConsensusSupport,
{
	let block_interval = meta.block_interval.expect("qed");
	let task = Scheduler::new(block_interval)
		.try_for_each(move |schedule_info| {
			work(schedule_info, support.clone())
				.map_err(|e| {
					warn!("Encountered consensus error: {:?}", e);
				})
				.or_else(|_| future::ready(Ok(())))
		})
		.then(|res| {
			if let Err(err) = res {
				warn!("Terminated with an error: {:?}", err);
			}
			future::ready(Ok(()))
		});
	task.await
}

async fn work<S>(schedule_info: ScheduleInfo, support: Arc<S>) -> CommonResult<()>
where
	S: ConsensusSupport,
{
	let confirmed_number = match support.get_confirmed_number() {
		Ok(number) => number.expect("qed"),
		Err(e) => {
			warn!("Unable to get best number: {}", e);
			return Ok(());
		}
	};

	let number = confirmed_number + 1;
	let timestamp = schedule_info.timestamp;

	let txs = match support.get_transactions_in_txpool() {
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
		if validate_transaction(tx, &support, number).is_err() {
			invalid_txs.push(tx.clone());
			continue;
		}
		let is_meta = support.is_meta_tx(&*(&tx.tx)).expect("qed");
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

	let build_block_params = BuildBlockParams {
		number,
		timestamp,
		meta_txs,
		payload_txs,
	};

	let commit_block_params = support.build_block(build_block_params)?;

	support.commit_block(commit_block_params).await?;

	let tx_hash_set = txs
		.iter()
		.map(|x| x.tx_hash.clone())
		.collect::<HashSet<_>>();

	support.remove_transactions_in_txpool(&tx_hash_set)?;

	Ok(())
}

fn validate_transaction<S>(
	tx: &Arc<FullTransaction>,
	support: &Arc<S>,
	number: u64,
) -> CommonResult<()>
where
	S: ConsensusSupport,
{
	if support.get_transaction(&tx.tx_hash)?.is_some() {
		return Err(errors::ErrorKind::Duplicated(tx.tx_hash.clone()).into());
	}
	let witness = tx.tx.witness.as_ref().expect("qed");
	if witness.until < number {
		return Err(errors::ErrorKind::ExceedUntil(tx.tx_hash.clone()).into());
	}

	Ok(())
}

fn get_poa_meta<S: ConsensusSupport>(support: Arc<S>) -> CommonResult<module::poa::Meta> {
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
