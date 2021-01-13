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

//! Common lib for subcommands

use std::path::PathBuf;

use app_dirs::get_app_root;
use app_dirs::{AppDataType, AppInfo};
use structopt::StructOpt;

use primitives::errors::CommonResult;

pub mod config;
pub mod errors;
pub mod spec;

pub const NAME: &str = "wingchain";
pub const AUTHOR: &str = "Wingchain";
pub const CONFIG: &str = "config";
pub const DATA: &str = "data";
pub const DB: &str = "db";
pub const SPEC_FILE: &str = "spec.toml";
pub const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, StructOpt, Clone)]
pub struct SharedParams {
	#[structopt(long = "home", value_name = "PATH", parse(from_os_str))]
	pub home: Option<PathBuf>,

	#[structopt(long = "log", value_name = "LOG", help = "Log pattern")]
	pub log: Option<String>,
}

/// Get the default home path, under the os user data path
pub fn get_default_home() -> CommonResult<PathBuf> {
	let app_info = AppInfo {
		name: NAME,
		author: AUTHOR,
	};

	let home = get_app_root(AppDataType::UserData, &app_info).map_err(errors::ErrorKind::AppDir)?;
	Ok(home)
}

/// Get the config file path, <home>/config.toml
pub fn get_config_path(home: &PathBuf) -> PathBuf {
	home.join(CONFIG)
}

/// Get the config file path, <home>/spec.toml
pub fn get_data_path(home: &PathBuf) -> PathBuf {
	home.join(DATA)
}
