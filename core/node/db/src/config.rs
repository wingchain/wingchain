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

use crate::{columns, errors, DBConfig};
use std::cmp;

use primitives::errors::CommonResult;
use rocksdb::{BlockBasedOptions, DBPath, Options, ReadOptions, WriteOptions};
use std::collections::HashMap;

pub fn gen_block_opts(config: &DBConfig) -> CommonResult<BlockBasedOptions> {
	let mut opts = BlockBasedOptions::default();
	opts.set_block_size(16 * 1024);

	let cache_size = config.memory_budget / 3;
	if cache_size == 0 {
		opts.disable_cache();
	} else {
		let cache = rocksdb::Cache::new_lru_cache(cache_size as usize)
			.map_err(errors::ErrorKind::RocksDB)?;
		opts.set_block_cache(&cache);
		opts.set_cache_index_and_filter_blocks(true);
		opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
	}
	opts.set_bloom_filter(10, true);
	Ok(opts)
}

pub fn gen_col_memory_budgets(config: &DBConfig) -> HashMap<&str, u64> {
	let state_col_memory_budget = (config.memory_budget as f64 * 0.9) as u64;
	let other_col_memory_budget =
		(config.memory_budget - state_col_memory_budget) / (columns::COLUMN_NAMES.len() as u64 - 1);
	let col_memory_budgets = columns::COLUMN_NAMES
		.iter()
		.map(|&name| {
			let budget = if name == "payload_state" {
				state_col_memory_budget
			} else {
				other_col_memory_budget
			};
			(name, budget)
		})
		.collect::<HashMap<_, _>>();

	col_memory_budgets
}

pub fn gen_cf_opts(col_memory_budget: u64, block_opts: &BlockBasedOptions) -> Options {
	let mut opts = Options::default();

	opts.set_block_based_table_factory(block_opts);
	opts.set_level_compaction_dynamic_level_bytes(true);
	opts.optimize_level_style_compaction(col_memory_budget as usize);
	opts.set_target_file_size_base(64 * 1024 * 1024);
	opts.set_compression_per_level(&[]);

	opts
}

pub fn gen_db_opts(config: &DBConfig) -> CommonResult<Options> {
	let mut opts = Options::default();
	opts.set_report_bg_io_stats(true);
	opts.set_use_fsync(false);
	opts.create_if_missing(true);
	opts.set_max_open_files(512);
	opts.set_bytes_per_sync(1048576);
	opts.increase_parallelism(cmp::max(1, ::num_cpus::get() as i32 / 2));
	let db_paths = config
		.partitions
		.iter()
		.map(|partition| DBPath::new(&partition.path, partition.target_size))
		.collect::<Result<Vec<_>, _>>()
		.map_err(errors::ErrorKind::RocksDB)?;
	opts.set_db_paths(&db_paths);
	Ok(opts)
}

pub fn gen_write_opts() -> WriteOptions {
	WriteOptions::default()
}

pub fn gen_read_opts() -> ReadOptions {
	let mut opts = ReadOptions::default();
	opts.set_verify_checksums(false);

	opts
}
