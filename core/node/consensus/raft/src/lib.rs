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

//! Raft consensus
//! TODO rm
#![allow(dead_code)]

use node_consensus_base::support::ConsensusSupport;
use node_executor::module::raft::Meta;
use primitives::{Address, SecretKey};
use std::sync::Arc;

pub struct Raft<S>
where
	S: ConsensusSupport,
{
	stream: Arc<RaftStream<S>>,
	support: Arc<S>,
}

struct RaftStream<S>
where
	S: ConsensusSupport,
{
	support: Arc<S>,
	raft_meta: Meta,
	current_secret_key: Option<SecretKey>,
	current_address: Option<Address>,
}
