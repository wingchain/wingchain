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

//! Service to make chain, txpool, consensus, api work together
//!
//! Architecture:
//!
//!   +------------------------------------------------------------+
//!   |                           Service                          |
//!   +------------------------------------------------------------+
//!   +------------------------------------------------------------+
//!   |                            API                             |
//!   +------------------------------------------------------------+
//!   +---------------------------+  +-----------------------------+
//!   |          Consensus        |  |         Coordinator         |
//!   +---------------------------+  +-----------------------------+
//!   +---------------------------+  +-----------------------------+
//!   |           TxPool          |  |           Network           |
//!   +---------------------------+  +-----------------------------+
//!   +------------------------------------------------------------+
//!   |                           Chain                            |
//!   +------------------------------------------------------------+
//!   +------------------------------------------------------------+
//!   |                          Executor                          |
//!   +------------------------------------------------------------+
//!   +------------------------------------------------------------+
//!   |                          StateDB                           |
//!   +------------------------------------------------------------+
//!   +------------------------------------------------------------+
//!   |                            DB                              |
//!   +------------------------------------------------------------+
//!   +------------------------------------------------------------+
//!   |                          Crypto                            |
//!   +------------------------------------------------------------+

use std::path::PathBuf;
use std::sync::Arc;

use tokio::runtime::Runtime;
use tokio::time::Duration;

use node_api::support::DefaultApiSupport;
use node_api::Api;
use node_chain::Chain;
use node_consensus::Consensus;
use node_consensus_base::support::DefaultConsensusSupport;
use node_coordinator::support::DefaultCoordinatorSupport;
use node_coordinator::Coordinator;
use node_txpool::support::DefaultTxPoolSupport;
use node_txpool::TxPool;
use primitives::errors::CommonResult;

use crate::config::{get_chain_config, get_file_config, get_other_config};
use crate::errors::ErrorKind;

mod config;
pub mod errors;

pub struct ServiceConfig {
	/// Home path
	pub home: PathBuf,
	/// Agent version
	pub agent_version: String,
}

pub struct Service {
	#[allow(dead_code)]
	config: ServiceConfig,
	#[allow(dead_code)]
	chain: Arc<Chain>,
	#[allow(dead_code)]
	txpool: Arc<TxPool<DefaultTxPoolSupport>>,
	#[allow(dead_code)]
	api: Arc<Api<DefaultApiSupport>>,
	#[allow(dead_code)]
	consensus: Arc<Consensus<DefaultConsensusSupport>>,
	#[allow(dead_code)]
	coordinator: Arc<Coordinator<DefaultCoordinatorSupport>>,
}

impl Service {
	pub fn new(config: ServiceConfig) -> CommonResult<Self> {
		let file_config = get_file_config(&config.home)?;

		// init chain
		let chain_config = get_chain_config(&file_config, &config)?;
		let chain = Arc::new(Chain::new(chain_config)?);

		let other_config = get_other_config(&file_config, &config, chain.get_basic())?;
		// init txpool
		let txpool_support = Arc::new(DefaultTxPoolSupport::new(chain.clone()));
		let txpool = Arc::new(TxPool::new(other_config.txpool, txpool_support)?);

		// init consensus poa
		let consensus_support =
			Arc::new(DefaultConsensusSupport::new(chain.clone(), txpool.clone()));
		let consensus = Arc::new(Consensus::new(other_config.consensus, consensus_support)?);

		// init coordinator
		let coordinator_support = Arc::new(DefaultCoordinatorSupport::new(
			chain.clone(),
			txpool.clone(),
			consensus.clone(),
		));
		let coordinator = Arc::new(Coordinator::new(
			other_config.coordinator,
			coordinator_support,
		)?);

		// init api
		let api_support = Arc::new(DefaultApiSupport::new(
			chain.clone(),
			txpool.clone(),
			coordinator.clone(),
		));
		let api = Arc::new(Api::new(other_config.api, api_support));

		Ok(Self {
			config,
			chain,
			txpool,
			api,
			consensus,
			coordinator,
		})
	}
}

/// Start service daemon
pub fn start(config: ServiceConfig) -> CommonResult<()> {
	let rt = Runtime::new().map_err(ErrorKind::Runtime)?;
	rt.block_on(start_service(config))?;
	Ok(())
}

async fn start_service(config: ServiceConfig) -> CommonResult<()> {
	let service = Service::new(config)?;
	wait_shutdown().await;
	drop(service);
	tokio::time::sleep(Duration::from_millis(50)).await;
	Ok(())
}

async fn wait_shutdown() {
	#[cfg(windows)]
	let _ = tokio::signal::ctrl_c().await;
	#[cfg(unix)]
	{
		use tokio::signal::unix;
		let mut sigtun_int = unix::signal(unix::SignalKind::interrupt()).unwrap();
		let mut sigtun_term = unix::signal(unix::SignalKind::terminate()).unwrap();
		tokio::select! {
			_ = sigtun_int.recv() => {}
			_ = sigtun_term.recv() => {}
		}
	}
}
