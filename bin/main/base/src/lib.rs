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

use app_dirs::get_app_root;
use app_dirs::{AppDataType, AppInfo};
use structopt::StructOpt;

pub mod errors;
pub mod spec;

pub const NAME: &str = "wingchain";
pub const AUTHOR: &str = "Wingchain";
pub const CONFIG: &str = "config";
pub const DATA: &str = "data";
pub const SPEC_FILE: &str = "spec.toml";

#[derive(Debug, StructOpt, Clone)]
pub struct SharedParams {
	#[structopt(long = "home", value_name = "PATH", parse(from_os_str))]
	pub home: Option<PathBuf>,
}

pub fn get_default_home() -> errors::Result<PathBuf> {
	let app_info = AppInfo {
		name: NAME,
		author: AUTHOR,
	};

	let home = get_app_root(AppDataType::UserData, &app_info)?;
	Ok(home)
}

pub fn get_config_path(home: &PathBuf) -> PathBuf {
	home.join(CONFIG)
}

pub fn get_data_path(home: &PathBuf) -> PathBuf {
	home.join(DATA)
}
