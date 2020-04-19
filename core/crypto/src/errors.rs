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
use std::path::PathBuf;

use primitives::errors::{CommonError, CommonErrorKind, Display};

#[derive(Debug, Display)]
pub enum ErrorKind {
	#[display(fmt = "Invalid secret key")]
	InvalidSecretKey,

	#[display(fmt = "Invalid public key")]
	InvalidPublicKey,

	#[display(fmt = "Verification failed")]
	VerificationFailed,

	#[display(fmt = "Custom lib load failed: {:?}", _0)]
	CustomLibLoadFailed(PathBuf),

	#[display(fmt = "Invalid hash length: {}", _0)]
	InvalidHashLength(usize),

	#[display(fmt = "Invalid dsa length: {:?}", _0)]
	InvalidDsaLength((usize, usize, usize)),

	#[display(fmt = "Invalid address length: {}", _0)]
	InvalidAddressLength(usize),

	#[display(fmt = "Invalid name: {}", _0)]
	InvalidName(String),
}

impl Error for ErrorKind {}

impl From<ErrorKind> for CommonError {
	fn from(error: ErrorKind) -> Self {
		CommonError::new(CommonErrorKind::Crypto, Box::new(error))
	}
}
