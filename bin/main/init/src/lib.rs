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

//! Subcommand `init`
//! init the config file, spec file before running a node

use std::fs;
use std::path::Path;

use chrono::TimeZone;
use log::info;
use rand::{thread_rng, Rng};

use base::{config::Config, get_default_home, spec::Spec, CONFIG_FILE, SPEC_FILE};
use primitives::errors::CommonResult;

use crate::cli::InitOpt;

pub mod cli;
pub mod errors;

pub fn run(opt: InitOpt) -> CommonResult<()> {
	let home = match opt.shared_params.home {
		Some(home) => home,
		None => get_default_home()?,
	};

	if home.exists() {
		return Err(errors::ErrorKind::HomePathExists(home).into());
	}

	init_config(&home)?;
	init_data(&home)?;

	info!("Inited: home path: {:?}", home);

	Ok(())
}

fn init_config(home: &Path) -> CommonResult<()> {
	let config_path = base::get_config_path(home);

	fs::create_dir_all(&config_path).map_err(errors::ErrorKind::IO)?;

	init_spec_file(&config_path)?;
	init_config_file(&config_path)?;

	Ok(())
}

fn init_spec_file(config_path: &Path) -> CommonResult<()> {
	let template = &include_bytes!("./res/spec.toml")[..];

	let template = String::from_utf8_lossy(template);
	let template = template.replace("${CHAIN_ID}", &gen_chain_id());
	let template = template.replace("${TIME}", &gen_time());

	fs::write(config_path.join(SPEC_FILE), &template).map_err(errors::ErrorKind::IO)?;

	// test
	toml::from_str::<Spec>(&template).map_err(errors::ErrorKind::TomlDe)?;

	Ok(())
}

fn init_config_file(config_path: &Path) -> CommonResult<()> {
	let template = &include_bytes!("./res/config.toml")[..];

	let template = String::from_utf8_lossy(template).to_string();

	fs::write(config_path.join(CONFIG_FILE), &template).map_err(errors::ErrorKind::IO)?;

	// test
	toml::from_str::<Config>(&template).map_err(errors::ErrorKind::TomlDe)?;

	Ok(())
}

fn gen_chain_id() -> String {
	let mut rng = thread_rng();
	let mut gen_char = || rng.gen_range(b'a', b'z' + 1) as char;
	let chain_id: String = (0..8).map(|_| gen_char()).collect();
	let chain_id = format!("chain-{}", chain_id);
	chain_id
}

fn gen_time() -> String {
	let millis = chrono::Local::now().timestamp_millis();
	chrono::Local.timestamp_millis(millis).to_rfc3339()
}

fn init_data(home: &Path) -> CommonResult<()> {
	let data_path = base::get_data_path(home);

	fs::create_dir_all(data_path).map_err(errors::ErrorKind::IO)?;

	Ok(())
}
