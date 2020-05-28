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

//! API
//! including rpc, websocket etc.

use std::sync::Arc;

use crate::support::ApiSupport;

pub mod errors;
mod rpc;
pub mod support;

#[derive(Clone)]
pub struct ApiConfig {
	/// rpc address: ip:port
	pub rpc_addr: String,
	/// http server worker count
	/// 0 for cpu num
	pub rpc_workers: usize,
	/// connection count per worker
	pub rpc_maxconn: usize,
}

pub struct Api<S>
where
	S: ApiSupport,
{
	#[allow(dead_code)]
	support: Arc<S>,
}

impl<S> Api<S>
where
	S: ApiSupport + Send + Sync + 'static,
{
	pub fn new(config: ApiConfig, support: Arc<S>) -> Self {
		let api = Api {
			support: support.clone(),
		};

		rpc::start_rpc(&config, support);

		api
	}
}
