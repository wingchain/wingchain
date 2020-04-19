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

use trie_db::TrieError;

use primitives::errors::{CommonError, CommonErrorKind, Display};

#[derive(Debug, Display)]
pub enum ErrorKind {
	#[display(fmt = "Load hasher conflict: {}, {}", _0, _1)]
	LoadHasherConflict(String, String),

	#[display(fmt = "Load hasher fail")]
	LoadHasherFail,

	#[display(fmt = "Trie error: {}", _0)]
	Trie(String),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::StateDB, Box::new(error))
	}
}

pub fn parse_trie_error<T, E>(e: Box<TrieError<T, E>>) -> CommonError
where
	T: Debug,
	E: Debug,
{
	ErrorKind::Trie(format!("{}", e)).into()
}
