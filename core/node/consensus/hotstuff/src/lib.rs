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

//! Hotstuff consensus

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::type_complexity)]

use std::sync::Arc;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use log::info;
use parking_lot::RwLock;

use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{Consensus as ConsensusT, ConsensusInMessage, ConsensusOutMessage};
use node_consensus_primitives::CONSENSUS_HOTSTUFF;
use node_executor::module;
use node_executor_primitives::EmptyParams;
use primitives::codec;
use primitives::errors::CommonResult;
use primitives::{Address, BlockNumber, Header};

use crate::proof::Proof;
use crate::stream::HotStuffStream;
use crypto::address::Address as AddressT;
use node_executor::module::hotstuff::Authorities;

pub use crate::config::HotStuffConfig;

mod config;
pub mod errors;
pub mod proof;
mod protocol;
mod storage;
mod stream;
mod verifier;

pub struct HotStuff<S>
where
	S: ConsensusSupport,
{
	#[allow(dead_code)]
	support: Arc<S>,
	in_tx: UnboundedSender<ConsensusInMessage>,
	out_rx: RwLock<Option<UnboundedReceiver<ConsensusOutMessage>>>,
}

impl<S> ConsensusT for HotStuff<S>
where
	S: ConsensusSupport,
{
	type Config = HotStuffConfig;
	type Support = S;

	fn new(config: HotStuffConfig, support: Arc<S>) -> CommonResult<Self> {
		let hotstuff_meta = get_hotstuff_meta(&support, &0)?;

		let (in_tx, in_rx) = unbounded();
		let (out_tx, out_rx) = unbounded();

		HotStuffStream::spawn(support.clone(), hotstuff_meta, config, out_tx, in_rx)?;

		info!("Initializing consensus hotstuff");

		let hotstuff = HotStuff {
			support,
			in_tx,
			out_rx: RwLock::new(Some(out_rx)),
		};

		Ok(hotstuff)
	}

	fn verify_proof(&self, header: &Header, proof: &primitives::Proof) -> CommonResult<()> {
		let name = &proof.name;
		if name != CONSENSUS_HOTSTUFF {
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

		unimplemented!()
	}

	fn in_message_tx(&self) -> UnboundedSender<ConsensusInMessage> {
		self.in_tx.clone()
	}

	fn out_message_rx(&self) -> Option<UnboundedReceiver<ConsensusOutMessage>> {
		self.out_rx.write().take()
	}
}

fn get_hotstuff_meta<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<module::hotstuff::Meta> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"hotstuff".to_string(),
			"get_meta".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}

fn get_hotstuff_authorities<S: ConsensusSupport>(
	support: &Arc<S>,
	number: &BlockNumber,
) -> CommonResult<Authorities> {
	support
		.execute_call_with_block_number(
			number,
			None,
			"hotstuff".to_string(),
			"get_authorities".to_string(),
			EmptyParams,
		)
		.map(|x| x.expect("qed"))
}
