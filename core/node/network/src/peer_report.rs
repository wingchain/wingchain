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

use node_peer_manager::PeerReport;

pub const PEER_REPORT_DIAL_FAILURE: PeerReport = PeerReport::new(-2, "Dial failure");
pub const PEER_REPORT_PROTOCOL_ERROR: PeerReport = PeerReport::new(-2, "Protocol error");
