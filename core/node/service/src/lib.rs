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

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use parity_codec::Decode;

use crypto::address::AddressImpl;
use crypto::dsa::DsaImpl;
use crypto::hash::HashImpl;
use main_base::spec::Spec;
use main_base::SystemInitParam;
use node_db::DB;
use primitives::BlockNumber;

use crate::chain::{BlockProposal, Chain};
use toml::Value;

pub mod errors;
pub mod chain;

pub struct Config {
	pub home: PathBuf,
}

pub struct Service {
	config: Config,
	db: Option<Arc<DB>>,
	chain: Option<Arc<Chain>>,
}

impl Service {
	pub fn new(config: Config) -> errors::Result<Self> {
		Ok(Self {
			config,
			db: None,
			chain: None,
		})
	}

	pub fn start(&mut self) -> errors::Result<()> {
		self.init_db()?;

		self.init_chain()?;

		Ok(())
	}

	fn init_db(&mut self) -> errors::Result<()> {
		let db_path = self.config.home.join(main_base::DATA).join(main_base::DB);

		let db = Arc::new(DB::open(&db_path)?);
		self.db = Some(db.clone());

		Ok(())
	}

	fn init_chain(&mut self) -> errors::Result<()> {
		let best_number = self.get_best_number()?;

		if best_number.is_none() {
			// commit genesis block
			let spec = self.get_spec()?;

			let basic = &spec.basic;
			let hash = Arc::new(HashImpl::from_str(&basic.hash)?);
			let dsa = Arc::new(DsaImpl::from_str(&basic.dsa)?);
			let address = Arc::new(AddressImpl::from_str(&basic.address)?);

			let config = chain::Config {
				hash,
				dsa,
				address,
			};
			let chain = Chain::new(self.db.as_ref().expect("qed").clone(), config)?;
			self.chain = Some(Arc::new(chain));

			//let block_proposal = self.build_genesis_block_proposal(&spec);
		}

		Ok(())
	}

	fn get_best_number(&self) -> errors::Result<Option<BlockNumber>> {
		let db = self.db.as_ref().expect("qed");
		let best_number = db.get(node_db::columns::GLOBAL, node_db::global_key::BEST_NUMBER)?.and_then(|mut x| {
			Decode::decode(&mut &x[..])
		});
		Ok(best_number)
	}

	fn get_spec(&self) -> errors::Result<Spec> {
		let spec: Spec =
			toml::from_str(&fs::read_to_string(self.config.home.join(main_base::CONFIG).join(main_base::SPEC_FILE))?)?;
		Ok(spec)
	}

	fn build_genesis_block_proposal(&self, spec: &Spec) -> errors::Result<BlockProposal> {
		let tx = match spec.genesis.txs.get(0) {
			Some(tx) if tx.method == "system.init" => tx,
			_ => return Err(errors::ErrorKind::InvalidSpec.into()),
		};

		let param = match tx.params.get(0) {
			Some(Value::String(param)) => {
				match serde_json::from_str::<SystemInitParam>(param) {
					Ok(param) => param,
					_ => return Err(errors::ErrorKind::InvalidSpec.into()),
				}
			}
			_ => return Err(errors::ErrorKind::InvalidSpec.into()),
		};

		let chain_id = param.chain_id;
		let time = param.time;

		unimplemented!()
	}
}
