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

use node_network::PeerReport;

pub const PEER_REPORT_INVALID_TX: PeerReport = PeerReport::new(-10, "Invalid tx");
pub const PEER_REPORT_HANDSHAKE_FAILED: PeerReport = PeerReport::new_fatal("Handshake failed");
pub const PEER_REPORT_INVALID_BLOCK: PeerReport = PeerReport::new(-20, "Invalid block");
pub const PEER_REPORT_BLOCK_REQUEST_TIMEOUT: PeerReport =
	PeerReport::new(-4, "Block request timeout");
