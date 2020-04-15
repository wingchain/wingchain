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

use primitives::errors::CommonResult;
use service::Config;
use service::Service;

use crate::cli::NodeOpt;

pub mod cli;
pub mod errors;

pub fn run(opt: NodeOpt) -> CommonResult<()> {
	let home = match opt.shared_params.home {
		Some(home) => home,
		None => base::get_default_home()?,
	};

	if !home_inited(&home) {
		return Err(errors::ErrorKind::NotInited(home).into());
	}

	let config = Config { home };

	let mut service = Service::new(config)?;

	service.start()?;

	Ok(())
}

fn home_inited(home: &PathBuf) -> bool {
	home.exists()
}
