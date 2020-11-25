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
use node_executor::{module, Executor};
use primitives::errors::{CommonError, CommonResult};
use primitives::{Address, BlockNumber, BuildBlockParams, FullTransaction, Transaction};

use crate::errors;

pub fn build_genesis(spec: &Spec, executor: &Executor) -> CommonResult<BuildBlockParams> {
	let mut meta_txs = vec![];
	let mut payload_txs = vec![];

	let mut timestamp: Option<u64> = None;

	for tx in &spec.genesis.txs {
		let tx = build_tx(tx, &executor, &mut timestamp)?;
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
	timestamp: &mut Option<u64>,
) -> CommonResult<Transaction> {
	let module = &tx.module;
	let method = &tx.method;
	let params = &tx.params;

	match (module.as_str(), method.as_str()) {
		("system", "init") => {
			let params: module::system::InitParams = JsonParams(params).try_into()?;
			*timestamp = Some(params.timestamp);
			executor.build_tx(None, module.clone(), method.clone(), params)
		}
		("balance", "init") => {
			let params: module::balance::InitParams = JsonParams(params).try_into()?;
			executor.build_tx(None, module.clone(), method.clone(), params)
		}
		("solo", "init") => {
			let params: module::solo::InitParams = JsonParams(params).try_into()?;
			executor.build_tx(None, module.clone(), method.clone(), params)
		}
		_ => Err(errors::ErrorKind::Spec(format!(
			"unknown module or method: {}.{}",
			module, method
		))
		.into()),
	}
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
			pub endow: Vec<(String, u64)>,
		}
		let params = serde_json::from_str::<InitParams>(self.0)
			.map_err(|e| errors::ErrorKind::Spec(format!("invalid json: {:?}", e)))?;
		let endow = params
			.endow
			.into_iter()
			.map(|(address, balance)| {
				let address = Address(hex::decode(&address).map_err(|_| {
					errors::ErrorKind::Spec(format!("invalid address format: {}", address))
				})?);
				Ok((address, balance))
			})
			.collect::<CommonResult<Vec<_>>>()?;

		Ok(module::balance::InitParams { endow })
	}
}

impl<'a> TryInto<module::solo::InitParams> for JsonParams<'a> {
	type Error = CommonError;
	fn try_into(self) -> Result<module::solo::InitParams, Self::Error> {
		#[derive(Deserialize)]
		pub struct InitParams {
			pub block_interval: Option<u64>,
		}
		let params = serde_json::from_str::<InitParams>(self.0)
			.map_err(|e| errors::ErrorKind::Spec(format!("invalid json: {:?}", e)))?;
		let block_interval = params.block_interval;

		Ok(module::solo::InitParams { block_interval })
	}
}

#[cfg(test)]
mod tests {
	use crypto::address::AddressImpl;
	use crypto::dsa::DsaImpl;
	use crypto::hash::HashImpl;
	use main_base::spec::{Basic, Genesis};

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
	fn test_build_genesis_txs() {
		let spec = Spec {
			basic: Basic {
				hash: "blake2b_256".to_string(),
				dsa: "ed25519".to_string(),
				address: "blake2b_160".to_string(),
			},
			genesis: Genesis {
				txs: vec![
					Tx {
						module: "system".to_string(),
						method: "init".to_string(),
						params: r#"
							{
								"chain_id": "chain-test",
								"timestamp": "2020-04-16T23:46:02.189+08:00",
								"until_gap": 20
							}
						"#
						.to_string(),
					},
					Tx {
						module: "balance".to_string(),
						method: "init".to_string(),
						params: r#"
							{
								"endow": [
									["0001020304050607080900010203040506070809", 1]
								]
							}
						"#
						.to_string(),
					},
				],
			},
		};

		let executor = Executor::new(
			Arc::new(HashImpl::Blake2b256),
			Arc::new(DsaImpl::Ed25519),
			Arc::new(AddressImpl::Blake2b160),
		);

		let BuildBlockParams {
			number,
			timestamp,
			meta_txs,
			payload_txs,
		} = build_genesis(&spec, &executor).unwrap();

		assert_eq!(number, 0);
		assert_eq!(timestamp, 1587051962189);
		assert_eq!(meta_txs.len(), 1);
		assert_eq!(payload_txs.len(), 1);
	}
}
