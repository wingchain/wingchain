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

use std::borrow::Cow;
use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::iter;
use std::pin::Pin;

use async_std::sync::Arc;
use futures::stream::Fuse;
use futures::task::{Context, Poll};
use futures::StreamExt;
use futures::{Sink, Stream};
use futures_codec::{BytesMut, Framed};
use libp2p::core::UpgradeInfo;
use libp2p::swarm::protocols_handler::{InboundUpgradeSend, OutboundUpgradeSend};
use libp2p::swarm::NegotiatedSubstream;
use unsigned_varint::codec::UviBytes;

const MAX_HANDSHAKE_LEN: usize = 1024;

pub struct InProtocol {
	protocol_name: Cow<'static, [u8]>,
}

#[pin_project::pin_project]
pub struct InSubstream {
	#[pin]
	socket: Fuse<Framed<NegotiatedSubstream, UviBytes<io::Cursor<Vec<u8>>>>>,
	/// received handshake, can be taken
	received_handshake: Option<Vec<u8>>,
}

impl InSubstream {
	pub fn take_received_handshake(&mut self) -> Option<Vec<u8>> {
		self.received_handshake.take()
	}
}

impl InProtocol {
	pub fn new(protocol_name: Cow<'static, [u8]>) -> Self {
		Self { protocol_name }
	}
}

impl UpgradeInfo for InProtocol {
	type Info = Cow<'static, [u8]>;
	type InfoIter = iter::Once<Self::Info>;

	fn protocol_info(&self) -> Self::InfoIter {
		iter::once(self.protocol_name.clone())
	}
}

impl InboundUpgradeSend for InProtocol {
	type Output = InSubstream;
	type Error = libp2p::core::upgrade::ReadOneError;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

	fn upgrade_inbound(self, mut socket: NegotiatedSubstream, _info: Self::Info) -> Self::Future {
		Box::pin(async move {
			let received_handshake =
				match libp2p::core::upgrade::read_one(&mut socket, MAX_HANDSHAKE_LEN).await {
					Ok(v) => v,
					Err(e) => return Err(e),
				};
			let substream = InSubstream {
				socket: Framed::new(socket, UviBytes::default()).fuse(),
				received_handshake: Some(received_handshake),
			};
			Ok(substream)
		})
	}
}

pub struct OutProtocol {
	protocol_name: Cow<'static, [u8]>,
	handshake: Arc<Vec<u8>>,
}

#[pin_project::pin_project]
pub struct OutSubstream {
	#[pin]
	socket: Framed<NegotiatedSubstream, UviBytes<io::Cursor<Vec<u8>>>>,
	send_queue: VecDeque<Vec<u8>>,
}

impl OutSubstream {
	pub fn send_message(&mut self, message: Vec<u8>) {
		self.send_queue.push_back(message)
	}
}

impl OutProtocol {
	pub fn new(protocol_name: Cow<'static, [u8]>, handshake: Arc<Vec<u8>>) -> Self {
		Self {
			protocol_name,
			handshake,
		}
	}
}

impl UpgradeInfo for OutProtocol {
	type Info = Cow<'static, [u8]>;
	type InfoIter = iter::Once<Self::Info>;

	fn protocol_info(&self) -> Self::InfoIter {
		iter::once(self.protocol_name.clone())
	}
}

impl OutboundUpgradeSend for OutProtocol {
	type Output = OutSubstream;
	type Error = io::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

	fn upgrade_outbound(self, mut socket: NegotiatedSubstream, _info: Self::Info) -> Self::Future {
		Box::pin(async move {
			match libp2p::core::upgrade::write_with_len_prefix(&mut socket, &*self.handshake).await
			{
				Ok(_v) => (),
				Err(e) => return Err(e),
			};
			let substream = OutSubstream {
				socket: Framed::new(socket, UviBytes::default()),
				send_queue: VecDeque::with_capacity(16),
			};
			Ok(substream)
		})
	}
}

impl Stream for InSubstream {
	type Item = Result<BytesMut, io::Error>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		let mut this = self.project();
		Stream::poll_next(this.socket.as_mut(), cx)
	}
}

impl Stream for OutSubstream {
	type Item = Result<(), io::Error>;
	fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		let mut this = self.project();

		if this.send_queue.is_empty() {
			return Poll::Ready(Some(Ok(())));
		}

		match Sink::poll_ready(this.socket.as_mut(), cx) {
			Poll::Pending => return Poll::Pending,
			Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
			Poll::Ready(Ok(_)) => (),
		}

		while let Some(message) = this.send_queue.pop_front() {
			match this.socket.as_mut().start_send(io::Cursor::new(message)) {
				Err(e) => return Poll::Ready(Some(Err(e))),
				Ok(_) => (),
			}
		}

		match Sink::poll_flush(this.socket.as_mut(), cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
			Poll::Ready(Ok(_)) => Poll::Ready(Some(Ok(()))),
		}
	}
}
