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

use std::path::PathBuf;
use std::sync::Arc;

use node_chain::Chain;
use primitives::errors::CommonResult;

pub mod errors;

pub struct Config {
	pub home: PathBuf,
}

pub struct Service {
	config: Config,
	chain: Arc<Chain>,
}

impl Service {
	pub fn new(config: Config) -> CommonResult<Self> {
		let chain_config = node_chain::Config {
			home: config.home.clone(),
		};

		let chain = Arc::new(Chain::new(chain_config)?);
		Ok(Self { config, chain })
	}

	pub fn start(&mut self) -> CommonResult<()> {
		Ok(())
	}
}
