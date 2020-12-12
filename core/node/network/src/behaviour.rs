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

use libp2p::identify::{Identify, IdentifyEvent};
use libp2p::ping::{Ping, PingEvent};
use libp2p::swarm::NetworkBehaviourEventProcess;
use libp2p::NetworkBehaviour;

pub enum BehaviourOut {}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourOut", poll_method = "poll")]
pub struct Behaviour {
	ping: Ping,
	identify: Identify,
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for Behaviour {
	fn inject_event(&mut self, event: IdentifyEvent) {}
}

impl NetworkBehaviourEventProcess<PingEvent> for Behaviour {
	fn inject_event(&mut self, event: PingEvent) {}
}
