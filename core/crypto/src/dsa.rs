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

use crate::errors;

pub mod ed25519;

pub trait Dsa {
	type KeyPair;

	fn generate_key_pair(&self) -> errors::Result<Self::KeyPair>;

	fn key_pair_from_secret(&self, secret: &[u8]) -> errors::Result<Self::KeyPair>;
}
