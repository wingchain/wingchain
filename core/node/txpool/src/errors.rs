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
use primitives::Hash;

#[derive(Debug, Display)]
pub enum ErrorKind {
	#[display(fmt = "Exceed capacity: {}", _0)]
	ExceedCapacity(usize),

	#[display(fmt = "Duplicated tx: tx_hash: {:?}", _0)]
	Duplicated(Hash),

	#[display(fmt = "Insert error: tx_hash: {:?}", _0)]
	Insert(Hash),

	#[display(fmt = "Channel error: {:?}", _0)]
	Channel(Box<dyn Error + Send + Sync>),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::TxPool, Box::new(error))
	}
}
