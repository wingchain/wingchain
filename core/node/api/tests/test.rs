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

use node_api::support::ApiSupport;
use node_api::{Api, ApiConfig};
use std::sync::Arc;

#[tokio::test]
async fn test_api() {
	let config = ApiConfig {
		rpc_addr: "0.0.0.0:3109".to_string(),
		rpc_workers: 1,
		rpc_maxconn: 100,
	};
	let support = Arc::new(get_support());

	let api = Api::new(config, support);

	// tokio::signal::ctrl_c().await;
}

struct TestApiSupport;

impl ApiSupport for TestApiSupport {
	fn test(&self) {}
}

fn get_support() -> TestApiSupport {
	TestApiSupport
}
