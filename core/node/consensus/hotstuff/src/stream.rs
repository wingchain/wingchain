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

use node_consensus_base::support::ConsensusSupport;
use std::sync::Arc;
use node_executor::module::hotstuff::Meta;
use crate::HotStuffConfig;
use futures::channel::mpsc::{UnboundedSender, UnboundedReceiver};
use node_consensus_base::{ConsensusOutMessage, ConsensusInMessage};
use primitives::errors::CommonResult;

pub struct HotStuffStream<S>
    where
        S: ConsensusSupport,
{
    /// External support
    support: Arc<S>,
}

impl<S> HotStuffStream<S>
    where
        S: ConsensusSupport,
{
    pub fn spawn(
        support: Arc<S>,
        hotstuff_meta: Meta,
        hotstuff_config: HotStuffConfig,
        out_tx: UnboundedSender<ConsensusOutMessage>,
        in_rx: UnboundedReceiver<ConsensusInMessage>,
    ) -> CommonResult<()> {
        unimplemented!()
    }
}
