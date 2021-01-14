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

//! Wingchain main CLI
//! Subcommands: init, node

use structopt::clap::{App, AppSettings};
use structopt::StructOpt;

use primitives::errors::CommonResult;

use crate::cli::{Opt, Subcommand};

mod cli;
mod errors;

fn main() {
	match run_main() {
		Ok(_) => (),
		Err(e) => eprintln!("{}", e),
	}
}

fn run_main() -> CommonResult<()> {
	let app = get_app();

	let opt = Opt::from_clap(&app.get_matches_from(std::env::args()));

	match opt.subcommand {
		None => {
			print_help();
			Ok(())
		}
		Some(subcommand) => run_subcommand(subcommand),
	}
}

fn run_subcommand(subcommand: Subcommand) -> CommonResult<()> {
	match subcommand {
		Subcommand::Init(opt) => {
			init_logger(&opt.shared_params.log)?;
			init::run(opt)?;
		}
		Subcommand::Node(opt) => {
			init_logger(&opt.shared_params.log)?;
			node::run(opt)?;
		}
	}
	Ok(())
}

fn get_app<'a, 'b>() -> App<'a, 'b> {
	let app = Opt::clap();

	let app = app
		.name(base::NAME)
		.bin_name(base::NAME)
		.version(env!("CARGO_PKG_VERSION"))
		.author(env!("CARGO_PKG_AUTHORS"))
		.about(env!("CARGO_PKG_DESCRIPTION"))
		.settings(&[
			AppSettings::GlobalVersion,
			AppSettings::ArgsNegateSubcommands,
			AppSettings::SubcommandsNegateReqs,
		]);
	app
}

fn print_help() {
	let mut app = get_app();
	app.print_help().expect("qed");
	println!()
}

fn init_logger(log: &Option<String>) -> CommonResult<()> {
	let mut builder = env_logger::Builder::new();

	builder.filter(None, log::LevelFilter::Info);

	if let Ok(rust_log) = std::env::var("RUST_LOG") {
		builder.parse_filters(&rust_log);
	}

	if let Some(log) = log {
		builder.parse_filters(&log);
	}

	builder.try_init().map_err(errors::ErrorKind::InitLogger)?;

	Ok(())
}
