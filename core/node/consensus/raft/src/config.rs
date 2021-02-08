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

use primitives::SecretKey;

pub const DEFAULT_INIT_EXTRA_ELECTION_TIMEOUT: u64 = 10000;
pub const DEFAULT_EXTRA_ELECTION_TIMEOUT_PER_KB: u64 = 5;
pub const DEFAULT_REQUEST_PROPOSAL_MIN_INTERVAL: u64 = 1000;

pub struct RaftConfig {
	pub secret_key: Option<SecretKey>,
	pub init_extra_election_timeout: Option<u64>,
	pub extra_election_timeout_per_kb: Option<u64>,
	pub request_proposal_min_interval: Option<u64>,
}
