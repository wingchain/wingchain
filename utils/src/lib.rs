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

use std::path::PathBuf;
use assert_cmd::cargo::cargo_bin;

#[cfg(target_os = "macos")]
pub fn get_dylib(package_name: &str) -> PathBuf {
    cargo_bin(format!("lib{}.dylib", package_name))
}

#[cfg(target_os = "linux")]
pub fn get_dylib(package_name: &str) -> PathBuf {
    cargo_bin(format!("lib{}.so", package_name))
}

#[cfg(target_os = "windows")]
pub fn get_dylib(package_name: &str) -> PathBuf {
    let path = cargo_bin(format!("{}.dll", package_name));
    let path = path.to_string_lossy();
    let path = path.trim_end_matches(".exe");
    PathBuf::from(path)
}