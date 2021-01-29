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

pub use scale_codec::{Decode, Encode};

use crate::errors::{CommonError, CommonErrorKind, CommonResult};

pub fn encode<E: Encode>(value: &E) -> CommonResult<Vec<u8>> {
	Ok(Encode::encode(value))
}

pub fn decode<D: Decode>(value: &mut &[u8]) -> CommonResult<D> {
	Decode::decode(value).map_err(|e| CommonError::new(CommonErrorKind::Codec, Box::new(e)))
}
