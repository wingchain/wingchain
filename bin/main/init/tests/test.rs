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

use std::fs;

use assert_cmd::Command;
use tempfile::tempdir;

use base::spec::Spec;

#[test]
fn test_init() {
	let home = tempdir().expect("could not create a temp dir");
	let home = home.into_path();

	// home should not exists
	fs::remove_dir(&home).unwrap();

	let mut cmd = Command::cargo_bin("wingchain").unwrap();

	let output = cmd.arg("init").arg("--home").arg(&home).output().unwrap();

	assert_eq!(output.status.success(), true);

	let spec: Spec =
		toml::from_str(&fs::read_to_string(home.join("config").join("spec.toml")).unwrap())
			.unwrap();

	assert_eq!(spec.genesis.txs[0].method, "system.set_chain_id");
	assert_eq!(
		spec.genesis.txs[0].params[0].as_str().map(str::len),
		Some(14)
	);

	assert_eq!(spec.genesis.txs[1].method, "system.set_time");
	assert!(spec.genesis.txs[1].params[0].as_datetime().is_some());
}
