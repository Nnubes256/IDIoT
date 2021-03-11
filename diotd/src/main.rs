#[macro_use]
extern crate tracing;

use std::{
    collections::HashMap,
    env::args,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{Context, Result};

use futures::prelude::*;
use libp2p::{
    core::transport::Boxed as BoxedTransport,
    identity::Keypair,
    kad::{
        record::Key, store::MemoryStore, AddProviderOk, Kademlia, KademliaEvent, PeerRecord,
        PutRecordOk, QueryResult, Quorum, Record,
    },
    mdns::{Mdns, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, SwarmBuilder},
    Multiaddr, NetworkBehaviour, PeerId, Swarm, Transport,
};

/// Network behavior for use with libp2p for each node
#[derive(NetworkBehaviour)]
struct DiotdBehavior {
    mdns: Mdns,
}

impl Debug for DiotdBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiotdBehavior")
            .field("mdns", &self.mdns)
            .finish()
    }
}

impl DiotdBehavior {
    pub async fn new(local_peer_id: PeerId) -> Result<Self> {
        let store = MemoryStore::new(local_peer_id);
        let mdns = Mdns::new().await.context("Couldn't initialize mDNS")?;
        Ok(Self { mdns })
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for DiotdBehavior {
    /// Called when `mdns` produces an event.
    #[instrument(skip(self))]
    fn inject_event(&mut self, event: MdnsEvent) {
        // If the event in question is the discovery of new peers through mDNS,
        // then add them to the Kademlia routing table
        if let MdnsEvent::Discovered(list) = event {
            for (peer_id, multiaddr) in list {
                debug!("Discovered peer {} with address {}", peer_id, multiaddr);
            }
        }
    }
}

/// Sets up the appropiate encryption keypair for this node
///
/// TODO: read keypair from persistent storage
#[instrument]
fn setup_keypair() -> Keypair {
    info!("Setting up keypair");
    Keypair::generate_ed25519()
}

type TransportImpl = BoxedTransport<(PeerId, libp2p::core::muxing::StreamMuxerBox)>;

/// Sets up the libp2p transport to use
#[instrument(skip(keypair))]
fn setup_transport(keypair: Keypair) -> Result<TransportImpl> {
    info!("Setting up transport");
    let transport = {
        let tcp = libp2p::tcp::TokioTcpConfig::new().nodelay(true);
        libp2p::dns::DnsConfig::new(tcp)?
    };

    let noise_keys = libp2p::noise::Keypair::<libp2p::noise::X25519Spec>::new()
        .into_authentic(&keypair)
        .expect("Signing libp2p-noise static DH keypair failed.");

    Ok(transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(libp2p::noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(libp2p::core::upgrade::SelectUpgrade::new(
            libp2p::yamux::YamuxConfig::default(),
            libp2p::mplex::MplexConfig::default(),
        ))
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let setter = args().len() > 1;

    let local_key = setup_keypair();
    let local_peer_id = PeerId::from(local_key.public());

    let transport = setup_transport(local_key).context("Failed to create the transport")?;

    let mut swarm = {
        let behavior = DiotdBehavior::new(local_peer_id)
            .await
            .context("Couldn't initialize the network behavior")?;
        SwarmBuilder::new(transport, behavior, local_peer_id)
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build()
    };

    let addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
    Swarm::listen_on(&mut swarm, addr)
        .context("Swarm was unable to start listening on the network")?;

    let mut timer = tokio::time::interval(Duration::from_secs(1));
    let val = AtomicU64::new(0);

    let mut listening = false;

    loop {
        tokio::select! {
            _ = timer.tick() => {

            }
            event = swarm.next() => {
                // All events are handled by the `NetworkBehaviourEventProcess`es.
                // I.e. the `swarm.next()` future drives the `Swarm` without ever
                // terminating.
                panic!("Unexpected event: {:?}", event);
            }
        }
        if !listening {
            for addr in Swarm::listeners(&swarm) {
                info!("Now listening on {:?}", addr);
                listening = true;
            }
        }
    }

    Ok(())
}
