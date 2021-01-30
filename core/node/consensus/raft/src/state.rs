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

use primitives::codec::{Decode, Encode};
use primitives::BlockNumber;

pub enum State {
	Leader,
	Candidate,
	Follower,
	Observer,
}

#[derive(Encode, Decode)]
pub struct Term {
	number: BlockNumber,
	id: u32,
}

impl Default for Term {
	fn default() -> Self {
		Self { number: 0, id: 0 }
	}
}

#[derive(Encode, Decode)]
pub struct LogIndex {
	number: BlockNumber,
	id: u32,
}

impl Default for LogIndex {
	fn default() -> Self {
		Self { number: 0, id: 0 }
	}
}
