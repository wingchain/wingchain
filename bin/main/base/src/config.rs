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

//! Scheme for config.toml

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
	pub txpool: TxPoolConfig,
	pub api: ApiConfig,
	pub validator: ValidatorConfig,
	pub network: NetworkConfig,
}

#[derive(Deserialize, Debug)]
pub struct TxPoolConfig {
	pub pool_capacity: usize,
}

#[derive(Deserialize, Debug)]
pub struct ApiConfig {
	pub rpc_addr: String,
	pub rpc_workers: usize,
	pub rpc_maxconn: usize,
}

#[derive(Deserialize, Debug)]
pub struct ValidatorConfig {
	pub secret_key_file: PathBuf,
}

#[derive(Deserialize, Debug)]
pub struct NetworkConfig {
	pub max_in_peers: u32,
	pub max_out_peers: u32,
	pub listen_addresses: Vec<String>,
	pub external_addresses: Vec<String>,
	pub bootnodes: Vec<String>,
	pub reserved_nodes: Vec<String>,
	pub reserved_only: bool,
	pub secret_key_file: PathBuf,
}
