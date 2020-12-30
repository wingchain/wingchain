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

use main_base::config::Config as FileConfig;
use node_api::ApiConfig;
use node_chain::Basic;
use node_consensus_poa::PoaConfig;
use node_txpool::TxPoolConfig;
use primitives::errors::CommonResult;

use crate::errors;
use crypto::dsa::Dsa;
use primitives::SecretKey;
use std::sync::Arc;

pub struct Config {
	pub txpool: TxPoolConfig,
	pub api: ApiConfig,
	pub poa: PoaConfig,
}

pub fn get_config(home: &PathBuf, basic: Arc<Basic>) -> CommonResult<Config> {
	let file_config = get_file_config(home)?;
	let config = Config {
		txpool: get_txpool_config(&file_config)?,
		api: get_api_config(&file_config)?,
		poa: get_poa_config(&file_config, home, basic)?,
	};
	Ok(config)
}

fn get_txpool_config(file_config: &FileConfig) -> CommonResult<TxPoolConfig> {
	let txpool = TxPoolConfig {
		pool_capacity: file_config.txpool.pool_capacity,
		buffer_capacity: file_config.txpool.buffer_capacity,
	};
	Ok(txpool)
}

fn get_api_config(file_config: &FileConfig) -> CommonResult<ApiConfig> {
	let api = ApiConfig {
		rpc_addr: file_config.api.rpc_addr.clone(),
		rpc_workers: file_config.api.rpc_workers,
		rpc_maxconn: file_config.api.rpc_maxconn,
	};
	Ok(api)
}

fn get_poa_config(
	file_config: &FileConfig,
	home: &PathBuf,
	basic: Arc<Basic>,
) -> CommonResult<PoaConfig> {
	let secret_key_file = {
		let secret_key_file = &file_config.validator.secret_key_file;
		let secret_key_file = if secret_key_file.starts_with("/") {
			secret_key_file.clone()
		} else {
			home.join(main_base::CONFIG).join(secret_key_file)
		};
		secret_key_file
	};

	let secret_key = {
		let secret_key = fs::read_to_string(&secret_key_file).map_err(|_| {
			errors::ErrorKind::Config(format!(
				"Failed to read secret key file: {:?}",
				secret_key_file
			))
		})?;
		let secret_key = hex::decode(secret_key.trim()).map_err(|_| {
			errors::ErrorKind::Config(format!("Invalid secret key in: {:?}", secret_key_file))
		})?;

		let _key_pair = basic
			.dsa
			.key_pair_from_secret_key(&secret_key)
			.map_err(|_| {
				errors::ErrorKind::Config(format!("Invalid secret key in: {:?}", secret_key_file))
			})?;

		SecretKey(secret_key)
	};

	let poa = PoaConfig { secret_key };
	Ok(poa)
}

fn get_file_config(home: &PathBuf) -> CommonResult<FileConfig> {
	let config_path = home.join(main_base::CONFIG).join(main_base::CONFIG_FILE);
	let config = fs::read_to_string(&config_path).map_err(|_| {
		errors::ErrorKind::Config(format!("Failed to read config file: {:?}", config_path))
	})?;

	let config = toml::from_str(&config)
		.map_err(|e| errors::ErrorKind::Config(format!("Failed to parse config file: {:?}", e)))?;

	Ok(config)
}
