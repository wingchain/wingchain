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

//! Architecture:
//!
//!   +------------------------------------------------------------+
//!   |                          Service                           |
//!   +------------------------------------------------------------+
//!            |                |                  |            |
//!   +--------------+   +-------------+   +----------------+   |
//!   |    Txpool    |   |     API     |   |    Consensus   |   |
//!   +--------------+   +-------------+   +----------------+   |
//!           |                 |                  |            |
//!   +------------------------------------------------------------+
//!   |                          Chain                             |
//!   +------------------------------------------------------------+
//!        |          |          |                    |
//!        |          |     +----------+        +----------+
//!        |          |     | Executor |        |  Network |
//!        |          |     +----------+        +----------+
//!        |          |     /
//!        |     +---------+
//!        |     | StateDB |
//!        |     +---------+
//!        |     /
//!   +---------+
//!   |    DB   |
//!   +---------+
//!   +------------------------------------------------------------+
//!   |                          Crypto                            |
//!   +------------------------------------------------------------+

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::runtime::Runtime;

use main_base::config::Config;
use node_api::support::DefaultApiSupport;
use node_api::{Api, ApiConfig};
use node_chain::{Chain, ChainConfig};
use node_txpool::{TxPool, TxPoolConfig};
use primitives::errors::CommonResult;
use node_consensus::support::DefaultConsensusSupport;
use node_consensus_solo::Solo;

pub mod errors;

pub struct ServiceConfig {
	pub home: PathBuf,
}

pub struct Service {
	#[allow(dead_code)]
	config: ServiceConfig,
	#[allow(dead_code)]
	chain: Arc<Chain>,
	#[allow(dead_code)]
	txpool: Arc<TxPool<Chain>>,
	#[allow(dead_code)]
	api: Arc<Api<DefaultApiSupport<Chain>>>,
	#[allow(dead_code)]
	consensus: Arc<Solo<DefaultConsensusSupport<Chain>>>,
}

impl Service {
	pub fn new(config: ServiceConfig) -> CommonResult<Self> {
		let file_config = get_config(&config.home)?;

		// init chain
		let chain_config = ChainConfig {
			home: config.home.clone(),
		};
		let chain = Arc::new(Chain::new(chain_config)?);

		// init txpool
		let txpool_config = TxPoolConfig {
			pool_capacity: file_config.txpool.pool_capacity,
			buffer_capacity: file_config.txpool.buffer_capacity,
		};
		let txpool = Arc::new(TxPool::new(txpool_config, chain.clone())?);

		// init api
		let api_config = ApiConfig {
			rpc_addr: file_config.api.rpc_addr,
			rpc_workers: file_config.api.rpc_workers,
			rpc_maxconn: file_config.api.rpc_maxconn,
		};
		let api_support = Arc::new(DefaultApiSupport::new(chain.clone(), txpool.clone()));
		let api = Arc::new(Api::new(api_config, api_support));

		// init consensus solo
		let consensus_support = Arc::new(DefaultConsensusSupport::new(chain.clone(), txpool.clone()));
		let consensus = Arc::new(Solo::new(consensus_support)?);

		Ok(Self {
			config,
			chain,
			txpool,
			api,
			consensus,
		})
	}
}

pub fn start(config: ServiceConfig) -> CommonResult<()> {
	let mut rt = Runtime::new().map_err(|e| errors::ErrorKind::Runtime(e))?;

	rt.block_on(start_service(config))?;

	Ok(())
}

async fn start_service(config: ServiceConfig) -> CommonResult<()> {
	Service::new(config)?;

	wait_shutdown().await;

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

fn get_config(home: &PathBuf) -> CommonResult<Config> {
	let config_path = home.join(main_base::CONFIG).join(main_base::CONFIG_FILE);
	let config = fs::read_to_string(&config_path).map_err(|_| {
		errors::ErrorKind::Config(format!("failed to read config file: {:?}", config_path))
	})?;

	let config = toml::from_str(&config)
		.map_err(|e| errors::ErrorKind::Config(format!("failed to parse config file: {:?}", e)))?;

	Ok(config)
}
