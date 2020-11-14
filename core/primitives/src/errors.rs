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
use std::sync::Arc;

pub use derive_more::{Constructor, Display};

#[derive(Debug, Display, Clone)]
pub enum CommonErrorKind {
	Main,
	Service,
	Crypto,
	DB,
	StateDB,
	Executor,
	TxPool,
	Chain,
	Codec,
	Api,
	Consensus,
	VM,
}

#[derive(Debug, Display, Clone)]
#[display(fmt = "[CommonError] Kind: {}, Error: {}", kind, error)]
pub struct CommonError {
	pub kind: CommonErrorKind,
	pub error: Arc<dyn Error + Send + Sync>,
}

impl CommonError {
	pub fn new(kind: CommonErrorKind, error: Box<dyn Error + Send + Sync>) -> Self {
		CommonError {
			kind,
			error: error.into(),
		}
	}
}

impl Error for CommonError {}

pub type CommonResult<T> = Result<T, CommonError>;
