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

use std::collections::HashSet;

use libp2p::PeerId;
use linked_hash_map::LinkedHashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use primitives::errors::CommonResult;

use crate::errors::ErrorKind;

mod errors;

pub struct PeerManagerConfig {
    pub max_in_peers: u32,
    pub max_out_peers: u32,
    pub bootnodes: HashSet<PeerId>,
    pub reserved: HashSet<PeerId>,
    pub reserved_only: bool,
}

#[derive(Clone, PartialEq)]
pub enum PeerState {
    In,
    Out,
}

#[derive(Clone, PartialEq)]
pub enum PeerType {
    Reserved,
    Normal,
}

pub struct PeerInfo {
    peer_state: PeerState,
    peer_type: PeerType,
}

pub struct IncomingId(pub u64);

pub enum Message {
    Connect(PeerId),
    Drop(PeerId),
    Accept(IncomingId),
    Reject(IncomingId),
}

pub struct PeerManager {
    /// config
    config: PeerManagerConfig,
    /// maintain active peers
    active: ActivePeers,
    /// maintain inactive peers
    inactive: InactivePeers,
    /// out messages sender
    tx: UnboundedSender<Message>,
    /// out message receiver
    rx: Option<UnboundedReceiver<Message>>,
}

impl PeerManager {
    pub fn new(config: PeerManagerConfig) -> CommonResult<Self> {
        let active = ActivePeers {
            peers: Default::default(),
            in_peers: 0,
            out_peers: 0,
        };
        let inactive = InactivePeers {
            reserved_peers: config.reserved.clone().into_iter().map(|x| (x, ())).collect(),
            normal_peers: config.bootnodes.clone().into_iter().map(|x| (x, ())).collect(),
        };

        let (tx, rx) = unbounded_channel();

        let mut peer_manager = Self {
            config,
            active,
            inactive,
            tx,
            rx: Some(rx),
        };
        peer_manager.activate()?;

        Ok(peer_manager)
    }

    pub fn subscribe(&mut self) -> CommonResult<UnboundedReceiver<Message>> {
        let rx = self.rx.take().ok_or(ErrorKind::Subscribe)?;
        Ok(rx)
    }

    pub fn discovered(&mut self, peer_id: PeerId) -> CommonResult<()> {
        if self.active.contains(&peer_id) {
            return Ok(());
        }
        self.inactive.insert_peer(peer_id, &self.config);
        self.activate()?;
        Ok(())
    }

    pub fn dropped(&mut self, peer_id: PeerId) -> CommonResult<()> {
        self.active.remove_peer(&peer_id);
        self.inactive.insert_peer(peer_id, &self.config);
        self.activate()?;
        Ok(())
    }

    pub fn incoming(&mut self, peer_id: PeerId, incoming_id: IncomingId) -> CommonResult<()> {
        if self.config.reserved_only {
            if !self.config.reserved.contains(&peer_id) {
                self.send(Message::Reject(incoming_id))?;
                return Ok(());
            }
        }

        let peer_type = match self.config.reserved.contains(&peer_id) {
            true => PeerType::Reserved,
            false => PeerType::Normal,
        };

        match self.active.insert_peer(peer_id, PeerState::In, peer_type, &self.config) {
            InsertPeerResult::Inserted(peer_id) => {
                self.inactive.remove_peer(peer_id, &self.config);
                self.send(Message::Accept(incoming_id))?;
            }
            InsertPeerResult::Replaced {
                inserted, removed
            } => {
                self.inactive.remove_peer(inserted, &self.config);
                self.inactive.insert_peer(removed.clone(), &self.config);
                self.send(Message::Accept(incoming_id))?;
                self.send(Message::Drop(removed))?;
            }
            InsertPeerResult::Exist(_peer_id) => {}
            InsertPeerResult::Full(peer_id) => {
                self.inactive.insert_peer(peer_id, &self.config);
                self.send(Message::Reject(incoming_id))?;
            }
        }

        Ok(())
    }

    fn activate(&mut self) -> CommonResult<()> {
        while let Some((peer_id, peer_type)) = self.inactive.take_peer(&self.config) {
            match self.active.insert_peer(peer_id, PeerState::Out, peer_type, &self.config) {
                InsertPeerResult::Inserted(peer_id) => {
                    self.send(Message::Connect(peer_id))?;
                }
                InsertPeerResult::Replaced {
                    inserted, removed
                } => {
                    self.inactive.insert_peer(removed.clone(), &self.config);
                    self.send(Message::Connect(inserted))?;
                    self.send(Message::Drop(removed))?;
                }
                InsertPeerResult::Exist(_peer_id) => {}
                InsertPeerResult::Full(peer_id) => {
                    self.inactive.insert_peer(peer_id, &self.config);
                    break;
                }
            }
        }
        Ok(())
    }

    fn send(&self, message: Message) -> CommonResult<()> {
        self.tx.send(message).map_err(|e| ErrorKind::Send(e.to_string()))?;
        Ok(())
    }
}

struct ActivePeers {
    peers: LinkedHashMap<PeerId, PeerInfo>,
    in_peers: u32,
    out_peers: u32,
}

impl ActivePeers {
    fn insert_peer(&mut self, peer_id: PeerId, peer_state: PeerState, peer_type: PeerType, config: &PeerManagerConfig) -> InsertPeerResult {
        if let Some(_peer_info) = self.peers.get(&peer_id) {
            return InsertPeerResult::Exist(peer_id);
        }
        let peers = match peer_state {
            PeerState::In => self.in_peers,
            PeerState::Out => self.out_peers,
        };
        if peers >= config.max_out_peers {
            if peer_type == PeerType::Reserved {
                let to_remove = self.peers.iter().find(|peer| {
                    peer.1.peer_state == peer_state && peer.1.peer_type == PeerType::Normal
                }).map(|(peer_id, peer_info)| (peer_id.clone(), peer_info.peer_state.clone()));

                if let Some((to_remove_peer_id, to_remove_peer_state)) = to_remove {

                    // insert
                    match peer_state {
                        PeerState::In => self.in_peers += 1,
                        PeerState::Out => self.out_peers += 1,
                    };
                    self.peers.insert(peer_id.clone(), PeerInfo {
                        peer_type,
                        peer_state,
                    });

                    // remove
                    match to_remove_peer_state {
                        PeerState::In => self.in_peers -= 1,
                        PeerState::Out => self.out_peers -= 1,
                    };
                    self.peers.remove(&to_remove_peer_id);

                    return InsertPeerResult::Replaced { inserted: peer_id, removed: to_remove_peer_id };
                }
            }
            return InsertPeerResult::Full(peer_id);
        }

        //insert
        match peer_state {
            PeerState::In => self.in_peers += 1,
            PeerState::Out => self.out_peers += 1,
        };
        self.peers.insert(peer_id.clone(), PeerInfo {
            peer_type,
            peer_state,
        });
        InsertPeerResult::Inserted(peer_id)
    }

    fn remove_peer(&mut self, peer_id: &PeerId) {
        let to_remove = self.peers.get(peer_id).map(|peer_info| peer_info.peer_state.clone());
        if let Some(to_remove_peer_state) = to_remove {
            match to_remove_peer_state {
                PeerState::In => self.in_peers -= 1,
                PeerState::Out => self.out_peers -= 1,
            };
            self.peers.remove(peer_id);
        }
    }

    fn contains(&self, peer_id: &PeerId) -> bool {
        self.peers.contains_key(peer_id)
    }
}

enum InsertPeerResult {
    Inserted(PeerId),
    Replaced {
        inserted: PeerId,
        removed: PeerId,
    },
    Exist(PeerId),
    Full(PeerId),
}

struct InactivePeers {
    reserved_peers: LinkedHashMap<PeerId, ()>,
    normal_peers: LinkedHashMap<PeerId, ()>,
}

impl InactivePeers {
    fn take_peer(&mut self, config: &PeerManagerConfig) -> Option<(PeerId, PeerType)> {
        if let Some((peer_id, _)) = self.reserved_peers.pop_front() {
            return Some((peer_id, PeerType::Reserved));
        }
        if config.reserved_only {
            return None;
        }
        if let Some((peer_id, _)) = self.normal_peers.pop_front() {
            return Some((peer_id, PeerType::Normal));
        }
        None
    }
    fn insert_peer(&mut self, peer_id: PeerId, config: &PeerManagerConfig) {
        let is_reserved = config.reserved.contains(&peer_id);
        match is_reserved {
            false => self.normal_peers.insert(peer_id, ()),
            true => self.reserved_peers.insert(peer_id, ()),
        };
    }
    fn remove_peer(&mut self, peer_id: PeerId, config: &PeerManagerConfig) {
        let is_reserved = config.reserved.contains(&peer_id);
        match is_reserved {
            false => self.normal_peers.remove(&peer_id),
            true => self.reserved_peers.remove(&peer_id),
        };
    }
}
