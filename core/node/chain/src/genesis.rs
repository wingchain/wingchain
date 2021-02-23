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

use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use chrono::DateTime;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use main_base::spec::{Spec, Tx};
use node_executor::{module, Context, Executor};
use primitives::codec::Encode;
use primitives::errors::{CommonError, CommonResult};
use primitives::types::ExecutionGap;
use primitives::{BlockNumber, BuildBlockParams, FullTransaction, Transaction};

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
		let is_meta = executor.is_meta_call(&tx.call)?;
		let tx_hash = executor.hash_transaction(&tx)?;
		let tx = FullTransaction { tx, tx_hash };
		match is_meta {
			true => meta_txs.push(Arc::new(tx)),
			false => payload_txs.push(Arc::new(tx)),
		}
	}

	let timestamp =
		timestamp.ok_or_else(|| errors::ErrorKind::Spec("No timestamp specified".to_string()))?;

	let number = 0;
	let execution_number = 0;

	Ok(BuildBlockParams {
		number,
		timestamp,
		meta_txs,
		payload_txs,
		execution_number,
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
			let module_params: module::system::InitParams =
				get_module_params::<SystemInitParams>(params)?.try_into()?;
			*timestamp = Some(module_params.timestamp);
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		("balance", "init") => {
			let module_params: module::balance::InitParams = get_module_params(params)?;
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		("poa", "init") => {
			let module_params: module::poa::InitParams = get_module_params(params)?;
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		("raft", "init") => {
			let module_params: module::raft::InitParams = get_module_params(params)?;
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		("contract", "init") => {
			let module_params: module::contract::InitParams = get_module_params(params)?;
			build_validate_tx(executor, context, module, method, module_params, params)
		}
		_ => Err(errors::ErrorKind::Spec(format!(
			"Unknown module or method: {}.{}",
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
	let call = executor.build_call(module.to_string(), method.to_string(), module_params)?;
	let tx = executor.build_tx(None, call).map_err(|e| {
		errors::ErrorKind::Spec(format!(
			"Invalid params for {}.{}: \n{} \ncause: {}",
			module, method, params, e
		))
	})?;

	executor.validate_tx(context, &tx, false).map_err(|e| {
		errors::ErrorKind::Spec(format!(
			"Invalid params for {}.{}: \n{} \ncause: {}",
			module, method, params, e
		))
	})?;
	Ok(tx)
}

#[derive(Deserialize)]
pub struct SystemInitParams {
	pub chain_id: String,
	pub timestamp: String,
	pub max_until_gap: BlockNumber,
	pub max_execution_gap: ExecutionGap,
	pub consensus: String,
}

impl TryFrom<SystemInitParams> for module::system::InitParams {
	type Error = CommonError;

	fn try_from(value: SystemInitParams) -> Result<Self, Self::Error> {
		let timestamp = DateTime::parse_from_rfc3339(&value.timestamp)
			.map_err(|e| errors::ErrorKind::Spec(format!("Invalid time format: {:?}", e)))?;
		let timestamp = timestamp.timestamp_millis() as u64;
		Ok(module::system::InitParams {
			chain_id: value.chain_id,
			timestamp,
			max_until_gap: value.max_until_gap,
			max_execution_gap: value.max_execution_gap,
			consensus: value.consensus,
		})
	}
}

fn get_module_params<P>(params: &str) -> CommonResult<P>
where
	P: DeserializeOwned,
{
	let params = serde_json::from_str::<P>(params)
		.map_err(|e| errors::ErrorKind::Spec(format!("Invalid json: {:?}", e)))?;
	Ok(params)
}

#[cfg(test)]
mod tests {
	use primitives::Address;

	use super::*;

	#[test]
	fn test_system_init_params() {
		let str = r#"
		{
			"chain_id": "chain-test",
			"timestamp": "2020-04-16T23:46:02.189+08:00",
			"max_until_gap": 20,
			"max_execution_gap": 8,
			"consensus": "poa"
		}
		"#;

		let param = get_module_params::<SystemInitParams>(str).unwrap();
		let param: module::system::InitParams = param.try_into().unwrap();

		assert_eq!(
			param,
			module::system::InitParams {
				chain_id: "chain-test".to_string(),
				timestamp: 1587051962189,
				max_until_gap: 20,
				max_execution_gap: 8,
				consensus: "poa".to_string(),
			}
		)
	}

	#[test]
	fn test_balance_init_params() {
		let str = r#"
		{
			"endow":[
				["0001020304050607080900010203040506070809", 1],
				["000102030405060708090001020304050607080a", 2]
			]
		}
		"#;

		let param = get_module_params::<module::balance::InitParams>(str).unwrap();

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
	fn test_poa_init_params() {
		let str = r#"
		{
			"block_interval": 1000,
			"admin": {
				"threshold": 1,
				"members": [["01020304", 1]]
			},
			"authority": "01020304"
		}
		"#;

		let param = get_module_params::<module::poa::InitParams>(str).unwrap();

		assert_eq!(
			param,
			module::poa::InitParams {
				block_interval: Some(1000),
				admin: module::poa::Admin {
					threshold: 1,
					members: vec![(Address::from_hex("01020304").unwrap(), 1)],
				},
				authority: Address::from_hex("01020304").unwrap(),
			}
		)
	}

	#[test]
	fn test_raft_init_params() {
		let str = r#"
		{
			"block_interval": 3000,
			"heartbeat_interval": 100,
			"election_timeout_min": 500,
			"election_timeout_max": 1000,
			"admin" : {
			    "threshold": 1,
				"members": [
				    ["0001020304050607080900010203040506070809", 1]
				]
			},
			"authorities": {
				"members": [
				    "0001020304050607080900010203040506070809",
					"000102030405060708090001020304050607080a"
				]
			}
		}
		"#;

		let param = get_module_params::<module::raft::InitParams>(str).unwrap();

		assert_eq!(
			param,
			module::raft::InitParams {
				block_interval: Some(3000),
				heartbeat_interval: 100,
				election_timeout_min: 500,
				election_timeout_max: 1000,
				admin: module::raft::Admin {
					threshold: 1,
					members: vec![(
						Address(vec![
							0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9
						]),
						1
					)],
				},
				authorities: module::raft::Authorities {
					members: vec![
						Address(vec![
							0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9
						]),
						Address(vec![
							0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 10
						]),
					]
				},
			}
		)
	}

	#[test]
	fn test_contract_init_params() {
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

		let param = get_module_params::<module::contract::InitParams>(str).unwrap();

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
