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

use primitives::errors::{CommonError, CommonErrorKind, Display};

#[derive(Debug, Display)]
pub enum ErrorKind {
	#[display(fmt = "Spec error: {}", _0)]
	Spec(String),

	#[display(fmt = "Data error: {}", _0)]
	Data(String),

	#[display(fmt = "Call error: {}", _0)]
	Call(String),

	#[display(fmt = "Execute queue error: {}", _0)]
	ExecuteQueue(String),

	#[display(fmt = "Channel error: {:?}", _0)]
	Channel(Box<dyn Error + Send + Sync>),

	#[display(fmt = "Commit block error: {}", _0)]
	CommitBlockError(CommitBlockError),

	#[display(fmt = "Validate tx error: {}", _0)]
	ValidateTxError(ValidateTxError),
}

#[derive(Debug, Display)]
pub enum CommitBlockError {
	/// Block duplicated
	#[display(fmt = "Duplicated")]
	Duplicated,
	/// Block is not the best
	#[display(fmt = "Not best")]
	NotBest,
}

#[derive(Debug, Display, Clone)]
pub enum ValidateTxError {
	#[display(fmt = "Duplicated tx: {}", _0)]
	DuplicatedTx(String),

	#[display(fmt = "Invalid tx until: {}", _0)]
	InvalidTxUntil(String),

	#[display(fmt = "Invalid tx witness: {}", _0)]
	InvalidTxWitness(String),

	#[display(fmt = "Invalid tx module: {}", _0)]
	InvalidTxModule(String),

	#[display(fmt = "Invalid tx method: {}", _0)]
	InvalidTxMethod(String),

	#[display(fmt = "Invalid tx params: {}", _0)]
	InvalidTxParams(String),

	#[display(fmt = "{}", _0)]
	Application(String),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::Chain, Box::new(error))
	}
}
