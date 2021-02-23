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

//! Raft consensus

#![allow(clippy::type_complexity)]

use std::sync::Arc;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use log::info;
use parking_lot::RwLock;

use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{Consensus as ConsensusT, ConsensusInMessage, ConsensusOutMessage};
use node_consensus_primitives::CONSENSUS_RAFT;
use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::codec;
use primitives::errors::CommonResult;
use primitives::{Address, BlockNumber, Header};

use crate::proof::Proof;
use crate::stream::RaftStream;
use crypto::address::Address as AddressT;
use node_executor::module::raft::Authorities;

pub use crate::config::RaftConfig;

mod config;
pub mod errors;
pub mod proof;
mod protocol;
mod storage;
mod stream;
mod verifier;

pub struct Raft<S>
where
	S: ConsensusSupport,
{
	#[allow(dead_code)]
	support: Arc<S>,
	in_tx: UnboundedSender<ConsensusInMessage>,
	out_rx: RwLock<Option<UnboundedReceiver<ConsensusOutMessage>>>,
}

impl<S> ConsensusT for Raft<S>
where
	S: ConsensusSupport,
{
	type Config = RaftConfig;
	type Support = S;

	fn new(config: RaftConfig, support: Arc<S>) -> CommonResult<Self> {
		let raft_meta = get_raft_meta(&support, &0)?;

		let (in_tx, in_rx) = unbounded();
		let (out_tx, out_rx) = unbounded();

		RaftStream::spawn(support.clone(), raft_meta, config, out_tx, in_rx)?;

		info!("Initializing consensus raft");

		let raft = Raft {
			support,
			in_tx,
			out_rx: RwLock::new(Some(out_rx)),
		};

		Ok(raft)
	}

	fn verify_proof(&self, header: &Header, proof: &primitives::Proof) -> CommonResult<()> {
		let name = &proof.name;
		if name != CONSENSUS_RAFT {
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

		let authorities = get_raft_authorities(&self.support, &(header.number - 1))?;
		let is_authority = authorities.members.contains(&address);
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

fn get_raft_meta<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<module::raft::Meta> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"raft".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}

fn get_raft_authorities<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<Authorities> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"raft".to_string(),
			"get_authorities".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}
