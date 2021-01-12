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
	PeerManager,
	Network,
	Coordinator,
}

#[derive(Debug, Display, Clone)]
#[display(fmt = "{} Error: {}", kind, error)]
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

pub trait Catchable<T> {
	fn or_else_catch<E: Error + 'static, O: FnOnce(&E) -> Option<Result<T, CommonError>>>(
		self,
		op: O,
	) -> CommonResult<T>;
}

impl<T> Catchable<T> for CommonResult<T> {
	fn or_else_catch<E: Error + 'static, O: FnOnce(&E) -> Option<Result<T, CommonError>>>(
		self,
		op: O,
	) -> CommonResult<T> {
		match self {
			Ok(v) => Ok(v),
			Err(e) => {
				if let Some(de) = e.error.downcast_ref::<E>() {
					match op(de) {
						Some(oe) => oe,
						None => Err(e),
					}
				} else {
					Err(e)
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::error::Error;

	use super::*;

	#[test]
	fn test_catchable() {
		#[derive(Debug, Display, Clone)]
		enum MainErrorKind {
			A1,
			#[allow(dead_code)]
			A2,
		}
		impl Error for MainErrorKind {};
		impl From<MainErrorKind> for CommonError {
			fn from(error: MainErrorKind) -> Self {
				CommonError::new(CommonErrorKind::Main, Box::new(error))
			}
		}

		#[derive(Debug, Display, Clone)]
		enum ServiceErrorKind {
			B1,
			B2,
		}
		impl Error for ServiceErrorKind {};
		impl From<ServiceErrorKind> for CommonError {
			fn from(error: ServiceErrorKind) -> Self {
				CommonError::new(CommonErrorKind::Service, Box::new(error))
			}
		}

		fn call_service_b1() -> CommonResult<()> {
			Err(ServiceErrorKind::B1.into())
		}

		fn call_service_b2() -> CommonResult<()> {
			Err(ServiceErrorKind::B2.into())
		}

		// will catch b1
		fn call_main_a1() -> CommonResult<()> {
			let _result = call_service_b1().or_else_catch::<ServiceErrorKind, _>(|e| match e {
				ServiceErrorKind::B1 => Some(Err(MainErrorKind::A1.into())),
				_ => None,
			})?;
			Ok(())
		}

		// will catch b1 and ignore the error
		fn call_main_a1_2() -> CommonResult<()> {
			let _result = call_service_b1().or_else_catch::<ServiceErrorKind, _>(|e| match e {
				ServiceErrorKind::B1 => Some(Ok(())),
				_ => None,
			})?;
			Ok(())
		}

		// will not catch b2
		fn call_main_a2() -> CommonResult<()> {
			let _result = call_service_b2()?;
			Ok(())
		}

		assert_eq!(format!("{}", call_main_a1().unwrap_err()), "Main Error: A1");
		assert!(call_main_a1_2().is_ok());
		assert_eq!(
			format!("{}", call_main_a2().unwrap_err()),
			"Service Error: B2"
		);
	}
}
