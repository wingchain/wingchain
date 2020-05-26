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

use std::sync::Arc;

use log::{debug, warn};
use tokio::sync::mpsc::{channel, Receiver, Sender};

use primitives::errors::CommonResult;
use primitives::{BlockNumber, BuildExecuteParams, FullTransaction, Hash};

use crate::backend::Backend;
use crate::errors;

#[derive(Debug)]
pub struct ExecuteTask {
	pub number: BlockNumber,
	pub timestamp: u64,
	pub block_hash: Hash,
	pub parent_hash: Hash,
	pub meta_state_root: Hash,
	pub payload_txs: Vec<Arc<FullTransaction>>,
}

pub struct ExecuteQueue {
	#[allow(dead_code)]
	backend: Arc<Backend>,
	task_tx: Sender<ExecuteTask>,
}

impl ExecuteQueue {
	pub fn new(backend: Arc<Backend>) -> Self {
		let (task_tx, task_rx) = channel(32);

		let execute_queue = Self {
			backend: backend.clone(),
			task_tx,
		};

		tokio::spawn(process_tasks(task_rx, backend));

		execute_queue
	}

	pub async fn insert_task(&self, task: ExecuteTask) -> CommonResult<()> {
		let result = self.task_tx.clone().send(task).await;
		result
			.map_err(|e| errors::ErrorKind::ExecuteQueue(format!("insert task error: {:?}", e)))?;
		Ok(())
	}
}

async fn process_tasks(mut task_rx: Receiver<ExecuteTask>, backend: Arc<Backend>) {
	loop {
		let task = task_rx.recv().await;
		if let Some(task) = task {
			match process_task(task, &backend) {
				Ok(_) => {}
				Err(e) => {
					warn!("Process task error: {}", e);
				}
			}
		}
	}
}

fn process_task(task: ExecuteTask, backend: &Arc<Backend>) -> CommonResult<()> {
	debug!("Execute task: {:?}", task);

	let number = task.number;
	let block_hash = task.block_hash.clone();

	let executed = backend.get_executed(&task.parent_hash).map_err(|e| {
		errors::ErrorKind::ExecuteQueue(format!(
			"Unable to get executed: block number: {}, block hash: {}, {}",
			number, block_hash, e
		))
	})?;

	let executed = match executed {
		Some(executed) => executed,
		None => {
			return Err(errors::ErrorKind::ExecuteQueue(format!(
				"Block not executed: block number: {}, block hash: {}",
				number, block_hash
			))
			.into())
		}
	};

	let build_execute_params = BuildExecuteParams {
		number: task.number,
		timestamp: task.timestamp,
		block_hash: task.block_hash,
		meta_state_root: task.meta_state_root,
		payload_state_root: executed.payload_executed_state_root,
		payload_txs: task.payload_txs,
	};

	let commit_execute_params = backend.build_execute(build_execute_params).map_err(|e| {
		errors::ErrorKind::ExecuteQueue(format!(
			"Build execute error: block number: {}, block hash: {}, {}",
			number, block_hash, e
		))
	})?;

	backend.commit_execute(commit_execute_params).map_err(|e| {
		errors::ErrorKind::ExecuteQueue(format!(
			"Commit execute error: block number: {}, block hash: {}, {}",
			number, block_hash, e
		))
	})?;

	Ok(())
}