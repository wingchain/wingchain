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

//! Build the genesis block according to the spec

use std::convert::TryInto;
use std::sync::Arc;

use chrono::DateTime;
use serde::Deserialize;

use main_base::spec::{Spec, Tx};
use node_executor::{module, Context, Executor};
use primitives::codec::Encode;
use primitives::errors::{CommonError, CommonResult};
use primitives::{Address, BlockNumber, BuildBlockParams, FullTransaction, Transaction};

use crate::errors;

pub fn build_genesis(
	spec: &Spec,
	executor: &Executor,
	context: &Context,
) -> CommonResult<BuildBlockParams> {
	let mut meta_txs = vec![];
	let mut payload_txs = vec![];

	let mut timestamp: Option<u64> = None;

	for tx in &spec.genesis.txs {
		let tx = build_tx(tx, &executor, context, &mut timestamp)?;
		let is_meta = executor.is_meta_tx(&tx)?;
		let tx_hash = executor.hash_transaction(&tx)?;
		let tx = FullTransaction { tx, tx_hash };
		match is_meta {
			true => meta_txs.push(Arc::new(tx)),
			false => payload_txs.push(Arc::new(tx)),
		}
	}

	let timestamp = timestamp.ok_or(errors::ErrorKind::Spec(
		"no timestamp specified".to_string(),
	))?;

	let number = 0;

	Ok(BuildBlockParams {
		number,
		timestamp,
		meta_txs,
		payload_txs,
	})
}

fn build_tx(
	tx: &Tx,
	executor: &Executor,
	context: &Context,
	timestamp: &mut Option<u64>,
) -> CommonResult<Transaction> {
	let module = &tx.module;
	let method = &tx.method;
	let params = &tx.params;

	match (module.as_str(), method.as_str()) {
		("system", "init") => {
			let module_params: module::system::InitParams = JsonParams(params).try_into()?;
			*timestamp = Some(module_params.timestamp);
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		("balance", "init") => {
			let module_params: module::balance::InitParams = JsonParams(params).try_into()?;
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		("poa", "init") => {
			let module_params: module::poa::InitParams = JsonParams(params).try_into()?;
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		("contract", "init") => {
			let module_params: module::contract::InitParams = JsonParams(params).try_into()?;
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		_ => Err(errors::ErrorKind::Spec(format!(
			"unknown module or method: {}.{}",
			module, method
		))
		.into()),
	}
}

fn build_validate_tx<P: Encode>(
	executor: &Executor,
	context: &Context,
	module: &str,
	method: &str,
	module_params: P,
	params: &str,
) -> CommonResult<Transaction> {
	let tx = executor
		.build_tx(None, module.to_string(), method.to_string(), module_params)
		.map_err(|e| {
			errors::ErrorKind::Spec(format!(
				"invalid params for {}.{}: \n{} \ncause: {}",
				module, method, params, e
			))
		})?;

	executor.validate_tx(context, &tx, false).map_err(|e| {
		errors::ErrorKind::Spec(format!(
			"invalid params for {}.{}: \n{} \ncause: {}",
			module, method, params, e
		))
	})?;
	Ok(tx)
}

struct JsonParams<'a>(&'a str);

impl<'a> TryInto<module::system::InitParams> for JsonParams<'a> {
	type Error = CommonError;
	fn try_into(self) -> Result<module::system::InitParams, Self::Error> {
		#[derive(Deserialize)]
		pub struct InitParams {
			pub chain_id: String,
			pub timestamp: String,
			pub until_gap: BlockNumber,
		}
		let params = serde_json::from_str::<InitParams>(self.0)
			.map_err(|e| errors::ErrorKind::Spec(format!("invalid json: {:?}", e)))?;
		let timestamp = DateTime::parse_from_rfc3339(&params.timestamp)
			.map_err(|e| errors::ErrorKind::Spec(format!("invalid time format: {:?}", e)))?;
		let timestamp = timestamp.timestamp_millis() as u64;
		let chain_id = params.chain_id;
		let until_gap = params.until_gap;
		Ok(module::system::InitParams {
			chain_id,
			timestamp,
			until_gap,
		})
	}
}

impl<'a> TryInto<module::balance::InitParams> for JsonParams<'a> {
	type Error = CommonError;
	fn try_into(self) -> Result<module::balance::InitParams, Self::Error> {
		#[derive(Deserialize)]
		pub struct InitParams {
			pub endow: Vec<(Address, u64)>,
		}
		let params = serde_json::from_str::<InitParams>(self.0)
			.map_err(|e| errors::ErrorKind::Spec(format!("invalid json: {:?}", e)))?;
		let endow = params.endow;

		Ok(module::balance::InitParams { endow })
	}
}

impl<'a> TryInto<module::poa::InitParams> for JsonParams<'a> {
	type Error = CommonError;
	fn try_into(self) -> Result<module::poa::InitParams, Self::Error> {
		#[derive(Deserialize)]
		pub struct InitParams {
			pub block_interval: Option<u64>,
			pub authority: Address,
		}
		let params = serde_json::from_str::<InitParams>(self.0)
			.map_err(|e| errors::ErrorKind::Spec(format!("invalid json: {:?}", e)))?;
		let block_interval = params.block_interval;
		let authority = params.authority;

		Ok(module::poa::InitParams {
			block_interval,
			authority,
		})
	}
}

impl<'a> TryInto<module::contract::InitParams> for JsonParams<'a> {
	type Error = CommonError;
	fn try_into(self) -> Result<module::contract::InitParams, Self::Error> {
		#[derive(Deserialize)]
		pub struct InitParams {
			pub max_stack_height: Option<u32>,
			pub initial_memory_pages: Option<u32>,
			pub max_memory_pages: Option<u32>,
			pub max_share_value_len: Option<u64>,
			pub max_share_size: Option<u64>,
			pub max_nest_depth: Option<u32>,
		}
		let params = serde_json::from_str::<InitParams>(self.0)
			.map_err(|e| errors::ErrorKind::Spec(format!("invalid json: {:?}", e)))?;

		Ok(module::contract::InitParams {
			max_stack_height: params.max_stack_height,
			initial_memory_pages: params.initial_memory_pages,
			max_memory_pages: params.max_memory_pages,
			max_share_value_len: params.max_share_value_len,
			max_share_size: params.max_share_size,
			max_nest_depth: params.max_nest_depth,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_into_system_init_params() {
		let str = r#"
		{
			"chain_id": "chain-test",
			"timestamp": "2020-04-16T23:46:02.189+08:00",
			"until_gap": 20
		}
		"#;
		let json_params = JsonParams(&str);

		let param: module::system::InitParams = json_params.try_into().unwrap();

		assert_eq!(
			param,
			module::system::InitParams {
				chain_id: "chain-test".to_string(),
				timestamp: 1587051962189,
				until_gap: 20,
			}
		)
	}

	#[test]
	fn test_into_balance_init_params() {
		let str = r#"
		{
			"endow":[
				["0001020304050607080900010203040506070809", 1],
				["000102030405060708090001020304050607080a", 2]
			]
		}
		"#;

		let json_params = JsonParams(&str);

		let param: module::balance::InitParams = json_params.try_into().unwrap();

		assert_eq!(
			param,
			module::balance::InitParams {
				endow: vec![
					(
						Address(vec![
							0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9
						]),
						1
					),
					(
						Address(vec![
							0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 10
						]),
						2
					)
				]
			}
		)
	}

	#[test]
	fn test_into_poa_init_params() {
		let str = r#"
		{
			"block_interval": 1000,
			"authority": "01020304"
		}
		"#;

		let json_params = JsonParams(&str);

		let param: module::poa::InitParams = json_params.try_into().unwrap();

		assert_eq!(
			param,
			module::poa::InitParams {
				block_interval: Some(1000),
				authority: Address::from_hex("01020304").unwrap(),
			}
		)
	}

	#[test]
	fn test_into_contract_init_params() {
		let str = r#"
		{
			"max_stack_height": 16384,
			"initial_memory_pages": 1024,
			"max_memory_pages": 2048,
			"max_share_value_len": 104857600,
			"max_share_size": 1024,
			"max_nest_depth": 8
		}
		"#;

		let json_params = JsonParams(&str);

		let param: module::contract::InitParams = json_params.try_into().unwrap();

		assert_eq!(
			param,
			module::contract::InitParams {
				max_stack_height: Some(16384),
				initial_memory_pages: Some(1024),
				max_memory_pages: Some(2048),
				max_share_value_len: Some(104857600),
				max_share_size: Some(1024),
				max_nest_depth: Some(8),
			}
		)
	}
}
