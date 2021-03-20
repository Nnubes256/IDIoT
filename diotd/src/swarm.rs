use std::{fmt::Debug, time::Duration};

use anyhow::{Context, Result};

use crate::store::PeerStore;
use serde::{Deserialize, Serialize};

use libp2p::{
    core::transport::Boxed as BoxedTransport,
    gossipsub::{
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, IdentTopic, MessageAuthenticity,
        ValidationMode,
    },
    identity::{ed25519::Keypair as Ed25519Keypair, Keypair},
    ping::{Ping, PingConfig, PingEvent},
    pnet::{PnetConfig, PreSharedKey},
    swarm::{
        ExpandedSwarm, IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourEventProcess,
        ProtocolsHandler, SwarmBuilder,
    },
    NetworkBehaviour, PeerId, Transport,
};
use libp2p_mdns::{Mdns, MdnsEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerData {
    pub(crate) name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiotdBroadcast {
    Identity(PeerData),
}

/// Network behavior for use with libp2p for each node
#[derive(NetworkBehaviour)]
pub struct DiotdBehavior {
    ping: Ping,
    mdns: Mdns,
    pub(crate) gossipsub: Gossipsub,
    #[behaviour(ignore)]
    peer_store: PeerStore,
}

impl Debug for DiotdBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiotdBehavior")
            .field("mdns", &self.mdns)
            .finish()
    }
}

impl DiotdBehavior {
    pub async fn new(local_peer_id: PeerId, db: &sled::Db, keypair: Keypair) -> Result<Self> {
        let peer_store =
            PeerStore::new(db, local_peer_id).context("Couldn't initialize peer store")?;

        let mdns = Mdns::with_service_name(b"_p2p-nodes-nope._udp.local".to_vec())
            .await
            .context("Couldn't initialize mDNS")?;

        let gossipsub_conf = GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(5))
            .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .build()
            .expect("Valid config");
        let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(keypair), gossipsub_conf)
            .expect("Couldn't initialize Gossipsub");

        let ping = Ping::new(PingConfig::new().with_keep_alive(true));

        gossipsub
            .subscribe(&IdentTopic::new("default"))
            .expect("Couldn't subscribe to topic");

        Ok(Self {
            mdns,
            peer_store,
            gossipsub,
            ping,
        })
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for DiotdBehavior {
    /// Called when `mdns` produces an event.
    #[instrument(skip(self))]
    fn inject_event(&mut self, event: MdnsEvent) {
        if let MdnsEvent::Discovered(list) = event {
            for (peer_id, multiaddr) in list {
                debug!("Discovered peer {} with address {}", peer_id, multiaddr);
                self.gossipsub.add_explicit_peer(&peer_id);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for DiotdBehavior {
    /// Called when `gossipsub` produces an event.
    #[instrument(skip(self, event))]
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Message {
                propagation_source: _,
                message_id: _,
                message,
            } => {
                let sender = if let Some(sender) = message.source {
                    sender
                } else {
                    warn!("Received broadcast message from null peer!");
                    return;
                };

                let msg: DiotdBroadcast = match bincode::deserialize(&message.data) {
                    Ok(msg) => msg,
                    Err(err) => {
                        error!("Error deserializing broadcast: {}", err);
                        return;
                    }
                };
                match msg {
                    DiotdBroadcast::Identity(peer_data) => {
                        info!("Updating peer identity: {} --> {:?}", sender, peer_data);
                        match self.peer_store.update_peer_data(&sender, &peer_data) {
                            Ok(_) => {}
                            Err(err) => error!("Error while updating peer identity: {}", err),
                        };
                    }
                }
            }
            GossipsubEvent::Subscribed { peer_id, topic } => {
                debug!("Peer {} subscribed to topic: {:?}", peer_id, topic);
            }
            GossipsubEvent::Unsubscribed { peer_id, topic } => {
                debug!("Peer {} unsubscribed from topic: {:?}", peer_id, topic);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for DiotdBehavior {
    #[instrument(skip(self, event))]
    fn inject_event(&mut self, event: PingEvent) {
        debug!("Ping: {:?}", event);
    }
}

/// Sets up the appropiate encryption keypair for this node
///
/// TODO: read keypair from persistent storage
#[instrument(skip(db))]
fn setup_keypair(db: &sled::Db) -> Result<Keypair> {
    info!("Setting up keypair");
    let cred_tree = db
        .open_tree("keystore")
        .context("Couldn't open keystore within database")?;

    let keypair = if let Some(mut key_raw) = cred_tree
        .get("self")
        .context("Couldn't get PKCS#8 keypair from storage")?
    {
        let keypair = Keypair::Ed25519(
            Ed25519Keypair::decode(key_raw.as_mut())
                .context("Couldn't decode Ed25519 key from database")?,
        );
        info!("Loaded keypair from database");
        keypair
    } else {
        info!("Keypair not found, generating");
        let new_keypair = Keypair::generate_ed25519();
        if let Keypair::Ed25519(ed25519_keypair) = &new_keypair {
            cred_tree
                .insert("self", &ed25519_keypair.encode()[..])
                .context("Couldn't insert newly-generated keypair into database")?;
        } else {
            unreachable!()
        }
        new_keypair
    };

    Ok(keypair)
}

type TransportImpl = BoxedTransport<(PeerId, libp2p::core::muxing::StreamMuxerBox)>;

/// Sets up the libp2p transport to use
#[instrument(skip(keypair, psk))]
async fn setup_transport(keypair: Keypair, psk: PreSharedKey) -> Result<TransportImpl> {
    info!("Setting up transport");
    let transport = {
        let tcp = libp2p::tcp::TokioTcpConfig::new().nodelay(true);
        let tcp_pnet = tcp.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket));
        libp2p::dns::DnsConfig::new(tcp_pnet)?
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

type DiodtSwarmIPH =
    <<DiotdBehavior as NetworkBehaviour>::ProtocolsHandler as IntoProtocolsHandler>::Handler;
pub type DiodtSwarm = ExpandedSwarm<
    DiotdBehavior,
    <DiodtSwarmIPH as ProtocolsHandler>::InEvent,
    <DiodtSwarmIPH as ProtocolsHandler>::OutEvent,
    <DiotdBehavior as NetworkBehaviour>::ProtocolsHandler,
>;

pub async fn setup_swarm(db: &sled::Db, psk: PreSharedKey) -> Result<DiodtSwarm> {
    let local_key = setup_keypair(db).context("Couldn't load local keypair")?;
    let local_peer_id = PeerId::from(local_key.public());
    info!("My peer ID is: {}", local_peer_id.to_base58());
    let transport = setup_transport(local_key.clone(), psk)
        .await
        .context("Failed to create the transport")?;
    let swarm = {
        let behavior = DiotdBehavior::new(local_peer_id, db, local_key)
            .await
            .context("Couldn't initialize the network behavior")?;
        SwarmBuilder::new(transport, behavior, local_peer_id)
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build()
    };
    Ok(swarm)
}
