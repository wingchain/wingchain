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
use std::fmt::Debug;

use primitives::errors::{CommonError, CommonErrorKind, Display};

#[derive(Debug, Display)]
pub enum ErrorKind {
	#[display(fmt = "Invalid txs: {:?}", _0)]
	InvalidTxs(String),

	#[display(fmt = "Invalid tx witness: {}", _0)]
	InvalidTxWitness(String),

	#[display(fmt = "Invalid tx module: {}", _0)]
	InvalidTxModule(String),

	#[display(fmt = "Invalid tx method: {}", _0)]
	InvalidTxMethod(String),

	#[display(fmt = "Invalid tx params: {}", _0)]
	InvalidTxParams(String),

	#[display(fmt = "Invalid address: {}", _0)]
	InvalidAddress(String),

	#[display(fmt = "Invalid: {}", _0)]
	Invalid(String),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::Executor, Box::new(error))
	}
}

pub type ModuleResult<T> = Result<T, ModuleError>;

pub type OpaqueModuleResult = ModuleResult<Vec<u8>>;

#[derive(Debug)]
pub enum ModuleError {
	/// System error, should not be accepted
	System(CommonError),
	/// Application error, should be accepted
	Application(String),
}

impl From<CommonError> for ModuleError {
	fn from(v: CommonError) -> Self {
		ModuleError::System(v)
	}
}

impl From<ErrorKind> for ModuleError {
	fn from(v: ErrorKind) -> Self {
		ModuleError::System(v.into())
	}
}

impl From<String> for ModuleError {
	fn from(v: String) -> Self {
		ModuleError::Application(v)
	}
}

impl From<&str> for ModuleError {
	fn from(v: &str) -> Self {
		ModuleError::Application(v.to_string())
	}
}
