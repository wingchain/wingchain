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
use std::sync::Arc;
use std::time::SystemTime;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::prelude::*;
use log::{error, info, trace};
use parking_lot::RwLock;

use crypto::address::Address as AddressT;
use crypto::dsa::{Dsa, KeyPair};
use node_consensus_base::{
	scheduler::ScheduleInfo, scheduler::Scheduler, support::ConsensusSupport,
	Consensus as ConsensusT, ConsensusConfig, ConsensusInMessage, ConsensusOutMessage,
};
use node_consensus_primitives::CONSENSUS_POA;
use node_executor::module;
use node_executor::module::poa::Meta;
use node_executor_primitives::EmptyParams;
use primitives::errors::CommonResult;
use primitives::{codec, Address, BlockNumber, Header, SecretKey};

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

		let secret_key = config.secret_key;

		let (in_tx, in_rx) = unbounded();
		let (out_tx, out_rx) = unbounded();

		PoaStream::spawn(support.clone(), poa_meta, secret_key, out_tx, in_rx)?;

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
		let proof: Proof = codec::decode(&mut &data[..]).map_err(|_| {
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
	secret_key: SecretKey,
	address: Address,
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
		secret_key: Option<SecretKey>,
		out_tx: UnboundedSender<ConsensusOutMessage>,
		in_rx: UnboundedReceiver<ConsensusInMessage>,
	) -> CommonResult<()> {
		let secret_key = match secret_key {
			Some(v) => v,
			None => return Ok(()),
		};
		let address = get_address(&secret_key, &support)?;

		let this = Self {
			support,
			poa_meta,
			secret_key,
			address,
			out_tx,
			in_rx,
		};
		tokio::spawn(this.start());
		Ok(())
	}

	async fn start(mut self) {
		info!("Start poa work");
		let mut scheduler = Scheduler::new(self.poa_meta.block_interval);
		loop {
			tokio::select! {
				Some(schedule_info) = scheduler.next() => {
					self.work(schedule_info)
					.unwrap_or_else(|e| error!("Poa stream handle work error: {}", e));
				}
				Some(in_message) = self.in_rx.next() => {
					self.on_in_message(in_message)
					.unwrap_or_else(|e| error!("Poa stream handle in message error: {}", e));
				}
			}
		}
	}

	fn work(&self, schedule_info: ScheduleInfo) -> CommonResult<()> {
		let current_state = &self.support.get_current_state();

		let authority_address = get_poa_authority(&self.support, &current_state.confirmed_number)?;
		let is_authority = self.address == authority_address;

		trace!(
			"Current node is authority: {}, authority address: {}, current address: {:?}",
			is_authority,
			authority_address,
			self.address
		);

		if !is_authority {
			return Ok(());
		}

		let build_block_params = self.support.prepare_block(schedule_info)?;
		let tx_hash_set = build_block_params
			.meta_txs
			.iter()
			.map(|x| x.tx_hash.clone())
			.chain(
				build_block_params
					.payload_txs
					.iter()
					.map(|x| x.tx_hash.clone()),
			)
			.collect::<HashSet<_>>();

		let mut commit_block_params = self.support.build_block(build_block_params)?;

		let current_secret_key = &self.secret_key;
		let proof = Proof::new(
			&commit_block_params.block_hash,
			current_secret_key,
			self.support.get_basic()?.dsa.clone(),
		)?;
		commit_block_params.proof = proof.try_into()?;

		self.support.commit_block(commit_block_params)?;

		self.support.txpool_remove_transactions(&tx_hash_set)?;

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
