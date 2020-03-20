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

use rocksdb::{BlockBasedOptions, Options, ReadOptions, WriteOptions};

const DB_DEFAULT_MEMORY_BUDGET_MB: usize = 128;

pub fn gen_block_opts() -> BlockBasedOptions {
	let mut opts = BlockBasedOptions::default();
	opts.set_block_size(16 * 1024);
	opts.set_lru_cache(DB_DEFAULT_MEMORY_BUDGET_MB * 1024 * 1024 / 3);
	opts.set_cache_index_and_filter_blocks(true);
	opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
	opts.set_bloom_filter(10, true);
	opts
}

pub fn gen_cf_opts(col_count: usize, block_opts: &BlockBasedOptions) -> Options {
	let mut opts = Options::default();

	opts.set_block_based_table_factory(block_opts);
	opts.set_level_compaction_dynamic_level_bytes(true);
	opts.optimize_level_style_compaction(memory_budget_per_col(col_count));
	opts.set_target_file_size_base(64 * 1024 * 1024);
	opts.set_compression_per_level(&[]);

	opts
}

pub fn gen_db_opts(col_count: usize) -> Options {
	let mut opts = Options::default();
	opts.set_report_bg_io_stats(true);
	opts.set_use_fsync(false);
	opts.create_if_missing(true);
	opts.set_max_open_files(512);
	opts.set_bytes_per_sync(1048576);
	opts.set_write_buffer_size(memory_budget_per_col(col_count) / 2);
	opts.increase_parallelism(cmp::max(1, ::num_cpus::get() as i32 / 2));

	opts
}

pub fn gen_write_opts() -> WriteOptions {
	WriteOptions::default()
}

pub fn gen_read_opts() -> ReadOptions {
	let mut opts = ReadOptions::default();
	opts.set_verify_checksums(false);

	opts
}

fn memory_budget_per_col(col_count: usize) -> usize {
	DB_DEFAULT_MEMORY_BUDGET_MB * 1024 * 1024 / col_count
}
