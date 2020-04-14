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

use error_chain::*;
use primitives::DispatchId;

error_chain! {
	foreign_links {
	}
	links {
		StateDB(node_statedb::errors::Error, node_statedb::errors::ErrorKind) #[doc="StateDB error"];
	}
	errors {
		InvalidDispatchId(dispatch_id: DispatchId) {
			description(""),
			display("Invalid dispatch id: {:?}", dispatch_id),
		}
		IllegalTxsPhase {
			description(""),
			display("Illegal txs phase, should not execute meta txs after payload txs"),
		}
		MixedTxs {
			description(""),
			display("Mixed txs, should not mix meta txs and payload txs in one batch"),
		}
	}
}
