use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    task::{Context as TaskContext, Poll},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use async_bincode::{AsyncBincodeReader, AsyncBincodeWriter};
use async_compat::CompatExt;
use diot_core::device::ActuationResult;
use futures::{sink::SinkExt, stream::StreamExt};

use crate::{
    hardware::{FullActuatorData, FullSensorData},
    store::RemotePeerDevice,
    system::{LocalPeerData, PeerSecrets},
};
use serde::{Deserialize, Serialize};

use libp2p::{
    core::{transport::Boxed as BoxedTransport, ProtocolName},
    gossipsub::{
        error::PublishError, Gossipsub, GossipsubConfigBuilder, GossipsubEvent, IdentTopic,
        MessageAuthenticity, ValidationMode,
    },
    identity::{ed25519::Keypair as Ed25519Keypair, Keypair},
    ping::{Ping, PingConfig, PingEvent},
    pnet::{PnetConfig, PreSharedKey},
    swarm::{
        ExpandedSwarm, IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction,
        NetworkBehaviourEventProcess, PollParameters, ProtocolsHandler, SwarmBuilder,
    },
    NetworkBehaviour, PeerId, Transport,
};
use libp2p_mdns::{Mdns, MdnsEvent};
use libp2p_request_response::{
    RequestId, RequestResponse, RequestResponseCodec, RequestResponseConfig, RequestResponseEvent,
    RequestResponseMessage, ResponseChannel,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerData {
    pub(crate) name: String,
    pub(crate) devices: HashMap<String, RemotePeerDevice>,
}

impl From<LocalPeerData> for PeerData {
    fn from(lpd: LocalPeerData) -> Self {
        let mut devices_2 = HashMap::with_capacity(lpd.devices.len());
        for (name, dev) in lpd.devices {
            devices_2.insert(name, dev.into());
        }
        PeerData {
            name: lpd.name,
            devices: devices_2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiotdBroadcast {
    Identity(PeerData),
    SensorData(FullSensorData),
}

#[derive(Debug)]
pub struct ReceivedBroadcast {
    pub sender: PeerId,
    pub broadcast_type: DiotdBroadcast,
}

#[derive(Debug)]
pub enum SwarmOutEvent {
    Broadcast(ReceivedBroadcast),
    ActuatorRequest {
        data: FullActuatorData,
        channel: ResponseChannel<RemoteActuationResponse>,
    },
    ActuatorResponse {
        id: RequestId,
        response: ActuationResult,
    },
}

// HACK: `bincode` can't serialize `serde` tagged enums; thus, we need a different type
// from it that we can then cast to `AcutationResult`

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RemoteActuationResponse {
    Success,
    Ignored,
    NoResponse,
    BadRequest {
        reason: String,
    },
    ActuatorError {
        error_code: i64,
        error_description: String,
    },
}

impl From<RemoteActuationResponse> for ActuationResult {
    fn from(remote: RemoteActuationResponse) -> Self {
        use RemoteActuationResponse::{ActuatorError, BadRequest, Ignored, NoResponse, Success};

        match remote {
            Success => Self::Success,
            Ignored => Self::Ignored,
            NoResponse => Self::NoResponse,
            BadRequest { reason } => Self::BadRequest { reason },
            ActuatorError {
                error_code,
                error_description,
            } => Self::ActuatorError {
                error_code,
                error_description,
            },
        }
    }
}

impl From<ActuationResult> for RemoteActuationResponse {
    fn from(remote: ActuationResult) -> Self {
        use ActuationResult::{ActuatorError, BadRequest, Ignored, NoResponse, Success};

        match remote {
            Success => Self::Success,
            Ignored => Self::Ignored,
            NoResponse => Self::NoResponse,
            BadRequest { reason } => Self::BadRequest { reason },
            ActuatorError {
                error_code,
                error_description,
            } => Self::ActuatorError {
                error_code,
                error_description,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActuatorRequestProtocol {
    V1,
}

impl ProtocolName for ActuatorRequestProtocol {
    fn protocol_name(&self) -> &[u8] {
        match *self {
            ActuatorRequestProtocol::V1 => b"/diodt/actuators/1.0",
        }
    }
}

#[derive(Clone)]
pub struct ActuatorRequestsCodec;

#[async_trait]
impl RequestResponseCodec for ActuatorRequestsCodec {
    type Protocol = ActuatorRequestProtocol;
    type Request = FullActuatorData;
    type Response = RemoteActuationResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let mut deserializer = AsyncBincodeReader::from(io.compat());
        match deserializer.next().await {
            Some(item) => match item {
                Ok(item) => Ok(item),
                Err(err) => {
                    error!("Error while deserializing GossipSub request: {}", err);
                    match *err {
                        bincode::ErrorKind::Io(err) => Err(err),
                        err => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
                    }
                }
            },
            None => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                anyhow!("Got None from request"),
            )),
        }
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let mut deserializer = AsyncBincodeReader::from(io.compat());
        match deserializer.next().await {
            Some(item) => match item {
                Ok(item) => Ok(item),
                Err(err) => {
                    error!("Error while deserializing GossipSub response: {}", err);
                    match *err {
                        bincode::ErrorKind::Io(err) => Err(err),
                        err => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
                    }
                }
            },
            None => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                anyhow!("Got None from request"),
            )),
        }
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let mut serializer = AsyncBincodeWriter::from(io.compat()).for_async();
        match serializer.send(req).await {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("Error while serializing GossipSub request: {}", err);
                match *err {
                    bincode::ErrorKind::Io(err) => Err(err),
                    err => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
                }
            }
        }
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let mut serializer = AsyncBincodeWriter::from(io.compat()).for_async();
        match serializer.send(res).await {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("Error while serializing GossipSub response: {}", err);
                match *err {
                    bincode::ErrorKind::Io(err) => Err(err),
                    err => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
                }
            }
        }
    }
}

/// Network behavior for use with libp2p for each node
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "SwarmOutEvent", poll_method = "poll")]
pub struct DiotdBehavior {
    ping: Ping,
    mdns: Mdns,
    pub(crate) gossipsub: Gossipsub,
    actuator_requests: RequestResponse<ActuatorRequestsCodec>,
    #[behaviour(ignore)]
    local_peer_id: PeerId,
    #[behaviour(ignore)]
    out_ev: VecDeque<SwarmOutEvent>,
}

impl Debug for DiotdBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiotdBehavior")
            .field("mdns", &self.mdns)
            .finish()
    }
}

impl DiotdBehavior {
    pub async fn new(local_peer_id: PeerId, keypair: Keypair) -> Result<Self> {
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

        let actuator_requests = RequestResponse::new(
            ActuatorRequestsCodec,
            std::iter::once((
                ActuatorRequestProtocol::V1,
                libp2p_request_response::ProtocolSupport::Full,
            )),
            RequestResponseConfig::default(),
        );

        gossipsub
            .subscribe(&IdentTopic::new("default"))
            .expect("Couldn't subscribe to topic");

        Ok(Self {
            ping,
            mdns,
            gossipsub,
            actuator_requests,
            local_peer_id,
            out_ev: VecDeque::new(),
        })
    }

    pub async fn broadcast_identity(&mut self, peer_data: PeerData) {
        //info!("Identity");
        let topic = IdentTopic::new("default");
        let message = bincode::serialize(&DiotdBroadcast::Identity(peer_data))
            .expect("Failed to serialize config?!");
        match self.gossipsub.publish(topic, message) {
            Ok(id) => debug!("Sent identity msg with ID: {}", id),
            Err(err) => match err {
                PublishError::InsufficientPeers => {}
                err => error!("Error while sending message: {:?}", err),
            },
        }
    }

    pub async fn broadcast_sensor_data(&mut self, sensor_data: FullSensorData) {
        //info!("Sensor data: {:?}", sensor_data);
        let topic = IdentTopic::new("default");
        let message = bincode::serialize(&DiotdBroadcast::SensorData(sensor_data))
            .expect("Failed to serialize config?!");
        match self.gossipsub.publish(topic, message) {
            Ok(id) => debug!("Sent sensor data msg with ID: {}", id),
            Err(err) => match err {
                PublishError::InsufficientPeers => {}
                err => error!("Error while sending message: {:?}", err),
            },
        }
    }

    pub async fn send_actuator_request(
        &mut self,
        peer: &PeerId,
        data: FullActuatorData,
    ) -> RequestId {
        self.actuator_requests.send_request(peer, data)
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    fn push_broadcast_event(&mut self, sender: PeerId, broadcast: DiotdBroadcast) {
        self.out_ev
            .push_back(SwarmOutEvent::Broadcast(ReceivedBroadcast {
                sender,
                broadcast_type: broadcast,
            }));
    }

    fn push_actuator_request_event(
        &mut self,
        data: FullActuatorData,
        channel: ResponseChannel<RemoteActuationResponse>,
    ) {
        self.out_ev
            .push_back(SwarmOutEvent::ActuatorRequest { data, channel });
    }

    fn push_actuator_response_event(&mut self, id: RequestId, response: ActuationResult) {
        self.out_ev
            .push_back(SwarmOutEvent::ActuatorResponse { id, response });
    }

    fn poll<TBehaviourIn>(
        &mut self,
        _: &mut TaskContext,
        _: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<TBehaviourIn, SwarmOutEvent>> {
        if let Some(ev) = self.out_ev.pop_front() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(ev));
        }
        Poll::Pending
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
                self.actuator_requests.add_address(&peer_id, multiaddr);
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
                self.push_broadcast_event(sender, msg);
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

impl NetworkBehaviourEventProcess<RequestResponseEvent<FullActuatorData, RemoteActuationResponse>>
    for DiotdBehavior
{
    #[instrument(skip(self, event))]
    fn inject_event(
        &mut self,
        event: RequestResponseEvent<FullActuatorData, RemoteActuationResponse>,
    ) {
        match event {
            RequestResponseEvent::Message { peer, message } => match message {
                RequestResponseMessage::Request {
                    request_id: _,
                    request,
                    channel,
                } => {
                    debug!("Received inbound request from peer {}", peer);
                    self.push_actuator_request_event(request, channel);
                }
                RequestResponseMessage::Response {
                    request_id,
                    response,
                } => {
                    debug!("Received response to outbound request from peer {}", peer);
                    self.push_actuator_response_event(request_id, response.into());
                }
            },
            RequestResponseEvent::OutboundFailure {
                peer,
                request_id,
                error,
            } => {
                error!(
                    "Outbound failure on Gossipsub, peer id = {}, request id = {}, err = {:?}",
                    peer, request_id, error
                );
            }
            RequestResponseEvent::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                error!(
                    "Inbound failure on Gossipsub, peer id = {}, request id = {}, err = {:?}",
                    peer, request_id, error
                );
            }
            RequestResponseEvent::ResponseSent { peer, request_id } => {
                debug!(
                    "Sent response for request id {} from peer {}",
                    request_id, peer
                );
            }
        }
    }
}

/// Sets up the appropiate encryption keypair for this node
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

type DiodtSwarmHandler =
    <<DiotdBehavior as NetworkBehaviour>::ProtocolsHandler as IntoProtocolsHandler>::Handler;
pub type DiodtSwarm = ExpandedSwarm<
    DiotdBehavior,
    <DiodtSwarmHandler as ProtocolsHandler>::InEvent,
    <DiodtSwarmHandler as ProtocolsHandler>::OutEvent,
    <DiotdBehavior as NetworkBehaviour>::ProtocolsHandler,
>;

pub async fn setup_swarm(secrets: PeerSecrets) -> Result<DiodtSwarm> {
    let local_key = Keypair::Ed25519(secrets.keypair);
    let local_peer_id = PeerId::from(local_key.public());
    info!("My peer ID is: {}", local_peer_id.to_base58());
    let transport = setup_transport(local_key.clone(), secrets.psk)
        .await
        .context("Failed to create the transport")?;
    let swarm = {
        let behavior = DiotdBehavior::new(local_peer_id, local_key)
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
