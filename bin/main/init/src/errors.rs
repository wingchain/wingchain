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

use std::error::Error;
use std::io;
use std::path::PathBuf;

use primitives::errors::{CommonError, CommonErrorKind, Display};

#[derive(Debug, Display)]
pub enum ErrorKind {
	#[display(fmt = "Home path already exists: {:?}", _0)]
	HomePathExists(PathBuf),

	#[display(fmt = "IO error: {:?}", _0)]
	IO(io::Error),

	#[display(fmt = "Toml deserialize error: {:?}", _0)]
	TomlDe(toml::de::Error),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::Main, Box::new(error))
	}
}
