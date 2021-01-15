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

//! Consensus

use node_consensus_base::support::ConsensusSupport;
use node_consensus_base::{Consensus as ConsensusT, ConsensusConfig};
use node_consensus_poa::Poa;
use node_consensus_primitives::CONSENSUS_POA;
use primitives::errors::CommonResult;
use std::sync::Arc;

pub enum Consensus<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	Poa(Poa<S>),
}

impl<S> Consensus<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	pub fn new(config: ConsensusConfig, support: Arc<S>) -> CommonResult<Self> {
		let system_meta = &support.get_current_state().system_meta;
		let consensus = system_meta.consensus.as_str();

		let consensus = match consensus {
			CONSENSUS_POA => {
				let poa = Poa::new(config, support)?;
				Self::Poa(poa)
			}
			other => {
				panic!("Unknown consensus: {}", other);
			}
		};

		Ok(consensus)
	}
}

impl<S> ConsensusT for Consensus<S>
where
	S: ConsensusSupport + Send + Sync + 'static,
{
	fn generate(&self) -> CommonResult<()> {
		match self {
			Consensus::Poa(poa) => poa.generate(),
		}
	}
}
