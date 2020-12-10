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

use std::time::Duration;

use futures::future::{select, Either};
use futures::StreamExt;
use libp2p::PeerId;
use linked_hash_map::LinkedHashMap;
use tokio::time::delay_for;

use node_peer_manager::{InMessage, IncomingId, OutMessage, PeerManager, PeerManagerConfig};

#[tokio::test]
async fn test_peer_manager_out_full() {
	let config = PeerManagerConfig {
		max_in_peers: 2,
		max_out_peers: 3,
		bootnodes: LinkedHashMap::new(),
		reserved: LinkedHashMap::new(),
		reserved_only: false,
	};
	let mut peer_manager = PeerManager::new(config);

	let peer_id_0 = PeerId::random();
	peer_manager.discovered(peer_id_0.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_0)));

	let peer_id_1 = PeerId::random();
	peer_manager.discovered(peer_id_1.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_1)));

	let peer_id_2 = PeerId::random();
	peer_manager.discovered(peer_id_2.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_2)));

	let peer_id_3 = PeerId::random();
	peer_manager.discovered(peer_id_3.clone());

	let message = select(peer_manager.next(), delay_for(Duration::from_millis(10))).await;
	match message {
		Either::Left(_) => panic!("should not get message"),
		Either::Right(_) => (),
	}
}

#[tokio::test]
async fn test_peer_manager_drop() {
	let config = PeerManagerConfig {
		max_in_peers: 2,
		max_out_peers: 3,
		bootnodes: LinkedHashMap::new(),
		reserved: LinkedHashMap::new(),
		reserved_only: false,
	};
	let mut peer_manager = PeerManager::new(config);

	let peer_id_0 = PeerId::random();
	peer_manager.discovered(peer_id_0.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_0.clone())));

	peer_manager.dropped(peer_id_0.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_0.clone())));
}

#[tokio::test]
async fn test_peer_manager_contains() {
	let config = PeerManagerConfig {
		max_in_peers: 2,
		max_out_peers: 3,
		bootnodes: LinkedHashMap::new(),
		reserved: LinkedHashMap::new(),
		reserved_only: false,
	};
	let mut peer_manager = PeerManager::new(config);

	let peer_id_0 = PeerId::random();
	peer_manager.discovered(peer_id_0.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_0.clone())));

	peer_manager.discovered(peer_id_0.clone());
	let message = select(peer_manager.next(), delay_for(Duration::from_millis(10))).await;
	match message {
		Either::Left(_) => panic!("should not get message"),
		Either::Right(_) => (),
	}
}

#[tokio::test]
async fn test_peer_manager_add_reserved() {
	let peer_id_0 = PeerId::random();
	let peer_id_1 = PeerId::random();
	let peer_id_2 = PeerId::random();

	let bootnodes = vec![peer_id_0.clone(), peer_id_1.clone(), peer_id_2.clone()]
		.into_iter()
		.map(|x| (x, ()))
		.collect();

	let config = PeerManagerConfig {
		max_in_peers: 2,
		max_out_peers: 3,
		bootnodes,
		reserved: LinkedHashMap::new(),
		reserved_only: false,
	};
	let mut peer_manager = PeerManager::new(config);

	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_0.clone())));
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_1.clone())));
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_2.clone())));

	let tx = peer_manager.tx();
	let peer_id_3 = PeerId::random();
	tx.send(InMessage::AddReservedPeer(peer_id_3.clone()))
		.map_err(|_| ())
		.unwrap();
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_3.clone())));

	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Drop(peer_id_0.clone())));
}

#[tokio::test]
async fn test_peer_manager_remove_reserved() {
	let peer_id_0 = PeerId::random();
	let peer_id_1 = PeerId::random();
	let peer_id_2 = PeerId::random();
	let peer_id_3 = PeerId::random();

	let reserved = vec![
		peer_id_0.clone(),
		peer_id_1.clone(),
		peer_id_2.clone(),
		peer_id_3.clone(),
	]
	.into_iter()
	.map(|x| (x, ()))
	.collect();

	let config = PeerManagerConfig {
		max_in_peers: 2,
		max_out_peers: 3,
		bootnodes: LinkedHashMap::new(),
		reserved,
		reserved_only: true,
	};
	let mut peer_manager = PeerManager::new(config);

	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_0.clone())));
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_1.clone())));
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_2.clone())));

	let tx = peer_manager.tx();
	tx.send(InMessage::RemoveReservedPeer(peer_id_1.clone()))
		.map_err(|_| ())
		.unwrap();
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Drop(peer_id_1.clone())));
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_3.clone())));
}

#[tokio::test]
async fn test_peer_manager_set_reserved() {
	let peer_id_0 = PeerId::random();
	let peer_id_1 = PeerId::random();
	let peer_id_2 = PeerId::random();
	let peer_id_3 = PeerId::random();

	let reserved = vec![peer_id_0.clone(), peer_id_1.clone()]
		.into_iter()
		.map(|x| (x, ()))
		.collect();

	let bootnodes = vec![peer_id_2.clone(), peer_id_3.clone()]
		.into_iter()
		.map(|x| (x, ()))
		.collect();

	let config = PeerManagerConfig {
		max_in_peers: 2,
		max_out_peers: 3,
		bootnodes,
		reserved,
		reserved_only: false,
	};
	let mut peer_manager = PeerManager::new(config);

	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_0.clone())));
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_1.clone())));
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Connect(peer_id_2.clone())));

	let tx = peer_manager.tx();
	tx.send(InMessage::SetReservedOnly(true))
		.map_err(|_| ())
		.unwrap();
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Drop(peer_id_2.clone())));
}

#[tokio::test]
async fn test_peer_manager_in_full() {
	let config = PeerManagerConfig {
		max_in_peers: 2,
		max_out_peers: 3,
		bootnodes: LinkedHashMap::new(),
		reserved: LinkedHashMap::new(),
		reserved_only: false,
	};
	let mut peer_manager = PeerManager::new(config);

	let peer_id_0 = PeerId::random();
	let incoming_id_0 = IncomingId(0);
	peer_manager.incoming(peer_id_0.clone(), incoming_id_0.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Accept(incoming_id_0.clone())));

	let peer_id_1 = PeerId::random();
	let incoming_id_1 = IncomingId(1);
	peer_manager.incoming(peer_id_1.clone(), incoming_id_1.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Accept(incoming_id_1.clone())));

	let peer_id_2 = PeerId::random();
	let incoming_id_2 = IncomingId(2);
	peer_manager.incoming(peer_id_2.clone(), incoming_id_2.clone());
	let message = peer_manager.next().await;
	assert_eq!(message, Some(OutMessage::Reject(incoming_id_2.clone())));
}
