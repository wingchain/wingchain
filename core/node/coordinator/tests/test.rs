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

use std::sync::Arc;
use crypto::dsa::DsaImpl;
use crypto::address::AddressImpl;
use node_network::{Keypair, Multiaddr, Protocol, LinkedHashMap, PeerId};
use utils_test::test_accounts;
use log::info;
use tokio::time::Duration;

mod base;

#[tokio::test]
async fn test_coordinator() {

    let _ = env_logger::try_init();

    let dsa = Arc::new(DsaImpl::Ed25519);
    let address = Arc::new(AddressImpl::Blake2b160);

    let (account1, account2) = test_accounts(dsa, address);

    let account1 = (account1.0, account1.1, account1.3);
    let account2 = (account2.0, account2.1, account2.3);

    let specs = vec![
        (account1.clone(), account1.clone(), Keypair::generate_ed25519(), 3409),
        (account1, account2, Keypair::generate_ed25519(), 3410),
    ];

    let bootnodes = {
        let bootnodes_spec = &specs[0];
        let bootnodes = (
            bootnodes_spec.2.public().into_peer_id(),
            Multiaddr::empty()
                .with(Protocol::Ip4([127, 0, 0, 1].into()))
                .with(Protocol::Tcp(bootnodes_spec.3)),
        );
        let bootnodes =
            std::iter::once((bootnodes, ())).collect::<LinkedHashMap<(PeerId, Multiaddr), ()>>();
        bootnodes
    };

    for spec in &specs {
        info!("peer id: {}", spec.2.public().into_peer_id());
    }

    let mut service = specs
        .iter()
        .map(|x| {
            let service = base::get_service(&x.0, &x.1,
                                            x.2.clone(), x.3, bootnodes.clone());
        })
        .collect::<Vec<_>>();

    tokio::time::sleep(Duration::from_secs(30)).await;
}
