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

use std::pin::Pin;
use std::sync::Arc;

use futures::task::{Context, Poll};
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use libp2p::bandwidth::BandwidthSinks;
use libp2p::swarm::SwarmEvent;
use libp2p::Swarm;
use log::{debug, error};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::behaviour::{Behaviour, BehaviourOut};
use crate::{NetWorkOutMessage, NetworkInMessage};

pub async fn start(mut stream: NetworkStream) {
	loop {
		stream.next().await;
	}
}

pub struct NetworkStream {
	pub swarm: Swarm<Behaviour>,
	#[allow(dead_code)]
	pub bandwidth: Arc<BandwidthSinks>,
	pub in_rx: UnboundedReceiver<NetworkInMessage>,
	pub out_tx: UnboundedSender<NetWorkOutMessage>,
}

impl Stream for NetworkStream {
	type Item = ();

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		// in_rx
		loop {
			match self.in_rx.poll_next_unpin(cx) {
				Poll::Ready(Some(out)) => match out {
					NetworkInMessage::SendMessage { peer_id, message } => {
						self.swarm.send_message(peer_id, message);
					}
				},
				Poll::Ready(None) => break,
				Poll::Pending => break,
			}
		}

		// swarm
		let out_tx = self.out_tx.clone();
		loop {
			let next_event = self.swarm.next_event();
			futures::pin_mut!(next_event);
			match next_event.poll_unpin(cx) {
				Poll::Ready(event) => match event {
					SwarmEvent::Behaviour(behaviour_out) => {
						let network_out_message = behaviour_out.into();
						out_tx
							.send(network_out_message)
							.unwrap_or_else(|e| error!("Network out message send error: {}", e));
					}
					_ => {
						debug!("Network event: {:?}", event);
					}
				},
				Poll::Pending => break,
			}
		}

		Poll::Pending
	}
}

impl From<BehaviourOut> for NetWorkOutMessage {
	fn from(v: BehaviourOut) -> Self {
		match v {
			BehaviourOut::ProtocolOpen {
				peer_id,
				connected_point,
			} => NetWorkOutMessage::ProtocolOpen {
				peer_id,
				connected_point,
			},
			BehaviourOut::ProtocolClose {
				peer_id,
				connected_point,
			} => NetWorkOutMessage::ProtocolClose {
				peer_id,
				connected_point,
			},
			BehaviourOut::Message { message } => NetWorkOutMessage::Message { message },
		}
	}
}
