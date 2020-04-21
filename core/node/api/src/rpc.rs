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

use std::cmp;
use std::sync::Arc;
use std::thread;

use jsonrpc_v2::{Data, Server};
use tokio::runtime::Runtime;

use primitives::errors::CommonResult;

use crate::errors;
use crate::rpc::method::{
	chain_get_block_by_hash, chain_get_block_by_number, chain_get_header_by_hash,
	chain_get_header_by_number, chain_get_raw_transaction_by_hash, chain_get_transaction_by_hash,
};
use crate::support::ApiSupport;
use crate::ApiConfig;

mod method;

pub fn start_rpc<S>(config: &ApiConfig, support: Arc<S>)
where
	S: ApiSupport + Send + Sync + 'static,
{
	let config = config.clone();
	thread::spawn(move || {
		let mut runtime = Runtime::new().expect("create http runtime");

		let local = tokio::task::LocalSet::new();

		local.block_on(&mut runtime, async {
			let local = tokio::task::LocalSet::new();

			let actix_rt = actix_rt::System::run_in_tokio("actix-web", &local);
			tokio::task::spawn_local(actix_rt);

			start_rpc_app(&config, support).await.expect("start api");
		});
	});
}

async fn start_rpc_app<S>(config: &ApiConfig, support: Arc<S>) -> CommonResult<()>
where
	S: ApiSupport + Send + Sync + 'static,
{
	let rpc = Server::new()
		.with_data(Data::new(support))
		.with_method("chain_getHeaderByNumber", chain_get_header_by_number::<S>)
		.with_method("chain_getHeaderByHash", chain_get_header_by_hash::<S>)
		.with_method("chain_getBlockByNumber", chain_get_block_by_number::<S>)
		.with_method("chain_getBlockByHash", chain_get_block_by_hash::<S>)
		.with_method(
			"chain_getTransactionByHash",
			chain_get_transaction_by_hash::<S>,
		)
		.with_method(
			"chain_getRawTransactionByHash",
			chain_get_raw_transaction_by_hash::<S>,
		)
		.finish();

	actix_web::HttpServer::new(move || {
		let rpc = rpc.clone();
		actix_web::App::new().service(
			actix_web::web::service("/")
				.guard(actix_web::guard::Post())
				.finish(rpc.into_web_service()),
		)
	})
	.workers(config.rpc_workers)
	.maxconn(cmp::max(config.rpc_maxconn / config.rpc_workers, 1))
	.bind(&config.rpc_addr)
	.map_err(|e| errors::ErrorKind::IO(e))?
	.run()
	.await
	.map_err(|e| errors::ErrorKind::IO(e))?;

	Ok(())
}
