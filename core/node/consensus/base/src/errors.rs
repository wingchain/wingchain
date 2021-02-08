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

use futures::channel::mpsc::TrySendError;
use primitives::errors::{CommonError, CommonErrorKind, Display};

#[derive(Debug, Display)]
pub enum ErrorKind {
	#[display(fmt = "Time error")]
	Time,

	#[display(fmt = "Verify proof error: {}", _0)]
	VerifyProofError(String),

	#[display(fmt = "Data error: {}", _0)]
	Data(String),

	#[display(fmt = "TxPool error: {}", _0)]
	TxPool(String),

	#[display(fmt = "Channel error: {:?}", _0)]
	Channel(Box<dyn Error + Send + Sync>),

	#[display(fmt = "Config error: {}", _0)]
	Config(String),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::Consensus, Box::new(error))
	}
}

pub fn map_channel_err<T: Send + Sync + 'static>(error: TrySendError<T>) -> CommonError {
	ErrorKind::Channel(Box::new(error)).into()
}
