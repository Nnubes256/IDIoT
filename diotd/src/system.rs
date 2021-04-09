use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, Result};

use libp2p::{identity::ed25519::Keypair, pnet::PreSharedKey, Multiaddr, PeerId, Swarm};
use tokio::{
    sync::{
        broadcast::{channel as broadcast_channel, Sender as BroadcastSender},
        oneshot,
    },
    task::JoinHandle,
};
use web::{WebserverConfig, WebserverMessage};

use crate::{
    control::{Action, ControlLayer, Rule},
    hardware::{FullSensorData, HardwareSupervisor, SupervisorOutEvent},
    store::{LocalPeerDevice, Storage},
    swarm::{setup_swarm, DiodtSwarm, DiotdBroadcast, ReceivedBroadcast, SwarmOutEvent},
    web,
};

use serde::{Deserialize, Serialize};

mod keypair_parse {
    use base64::STANDARD;
    use base64_serde::base64_serde_type;
    use libp2p::identity::ed25519::Keypair;
    use serde::{Deserializer, Serializer};

    base64_serde_type!(Base64Standard, STANDARD);

    pub fn serialize<S>(data: &Keypair, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw_data = data.encode();
        Base64Standard::serialize(&raw_data, ser)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Keypair, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut raw_data: Vec<u8> = Base64Standard::deserialize(de)?;
        Keypair::decode(&mut raw_data).map_err(serde::de::Error::custom)
    }
}

mod psk_parse {
    use std::str::FromStr;

    use libp2p::pnet::PreSharedKey;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &PreSharedKey, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw_data = data.to_string();
        ser.serialize_str(&raw_data)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<PreSharedKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw_data: String = Deserialize::deserialize(de)?;
        PreSharedKey::from_str(&raw_data).map_err(serde::de::Error::custom)
    }
}

pub mod peerid_opt_parse {
    use libp2p::PeerId;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Option<PeerId>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match data {
            Some(peer_id) => {
                let raw_data = peer_id.to_base58();
                ser.serialize_some(&raw_data)
            }
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Option<PeerId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b58_data: Option<String> = Deserialize::deserialize(de)?;
        b58_data
            .map(|x| {
                bs58::decode(&x)
                    .into_vec()
                    .map_err(serde::de::Error::custom)
            })
            .transpose()?
            .map(|raw_data| PeerId::from_bytes(&raw_data).map_err(serde::de::Error::custom))
            .transpose()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemConfig {
    pub peer: LocalPeerData,
    pub secrets: Option<PeerSecrets>,
    pub web: WebserverConfig,
    pub rules: Option<Vec<Rule>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerSecrets {
    #[serde(with = "psk_parse")]
    pub psk: PreSharedKey,
    #[serde(with = "keypair_parse")]
    pub keypair: Keypair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPeerData {
    pub(crate) name: String,
    pub(crate) devices: HashMap<String, LocalPeerDevice>,
}

pub struct System {
    swarm: DiodtSwarm,
    supervisor: HardwareSupervisor,
    storage: Arc<Storage>,
    config: SystemConfig,
    control: ControlLayer,
    webserver_task: Option<JoinHandle<()>>,
    webserver_tx: BroadcastSender<WebserverMessage>,
}

impl System {
    pub async fn from_config(mut config: SystemConfig) -> Result<Self> {
        let secrets = config.secrets.take().expect("secrets to be there");

        let supervisor = HardwareSupervisor::from_peer_data(config.peer.clone());

        let swarm: DiodtSwarm = setup_swarm(secrets).await.context("Couldn't setup swarm")?;

        let storage = Arc::new(
            Storage::new(swarm.local_peer_id(), config.peer.clone())
                .context("Couldn't open storage")?,
        );

        let (webserver_tx, _) = broadcast_channel(512);

        let control = ControlLayer::from_ruleset(config.rules.clone().unwrap_or_default());

        Ok(Self {
            swarm,
            supervisor,
            storage,
            config,
            control,
            webserver_task: None,
            webserver_tx,
        })
    }

    pub async fn launch(&mut self) -> Result<()> {
        self.supervisor
            .start_devices()
            .await
            .context("Failed to start devices")?;

        let addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
        Swarm::listen_on(&mut self.swarm, addr)
            .context("Swarm was unable to start listening on the network")?;

        self.webserver_task = Some(
            web::webserver_spawn(
                self.storage.clone(),
                self.webserver_tx.clone(),
                self.config.web.clone(),
            )
            .await,
        );
        self.system_loop().await;
        Ok(())
    }

    async fn system_loop(&mut self) {
        let mut timer = tokio::time::interval(Duration::from_secs(5));

        let mut listening = false;

        loop {
            tokio::select! {
                _ = timer.tick() => {
                    self.swarm.broadcast_identity(self.config.peer.clone().into()).await;
                }
                swarm_event = self.swarm.next() => {
                    self.handle_swarm_event(swarm_event).await;
                }
                Some(sensor_data) = self.supervisor.device_inbox.recv() => {
                    match sensor_data {
                        SupervisorOutEvent::SensorData(sensor_data) => {
                            self.swarm.broadcast_sensor_data(sensor_data.clone()).await;
                            self.handle_local_sensor_data(&sensor_data).await;
                        }
                    }
                }
            }
            if !listening {
                for addr in Swarm::listeners(&self.swarm) {
                    info!("Now listening on {:?}", addr);
                    listening = true;
                }
            }
        }
    }

    async fn handle_actions(&mut self, actions: Vec<Action>) {
        let local_peer_id = self.storage.local_peer_id();
        for action in actions {
            match action.node {
                Some(node) if node != local_peer_id => {
                    self.swarm
                        .send_actuator_request(&node, action.actuator)
                        .await;
                }
                _ => {
                    let (sender, receiver) = oneshot::channel();

                    tokio::spawn(async move {
                        match receiver.await {
                            Ok(res) => match res {
                                Ok(res) => info!("Received actuation result from control layer: {:?}", res),
                                Err(err) => error!("Error on actuation request triggered from local control layer: {}", err)
                            }
                            Err(send_err) => error!("Error while receiving result of actuation request from local control layer: {}", send_err)
                        }
                    });

                    if self
                        .supervisor
                        .actuate_device_local(action.actuator.clone(), sender)
                        .is_none()
                    {
                        warn!(
                            "Action attempted to triggered unknown actuator: {:?}",
                            action.actuator
                        )
                    };
                }
            }
        }
    }

    async fn handle_remote_sensor_data(&mut self, peer_id: PeerId, sensor_data: FullSensorData) {
        self.handle_sensor_data(peer_id, sensor_data.clone()).await;

        if let Some(actions) = self.control.trigger_remote(peer_id, &sensor_data) {
            self.handle_actions(actions).await;
        }
    }

    async fn handle_local_sensor_data(&mut self, sensor_data: &FullSensorData) {
        let local_peer_id = self.storage.local_peer_id();
        self.handle_sensor_data(local_peer_id, sensor_data.clone())
            .await;

        if let Some(actions) = self.control.trigger_local(sensor_data) {
            self.handle_actions(actions).await;
        }
    }

    async fn handle_sensor_data(&mut self, sender: PeerId, sensor_data: FullSensorData) {
        match self.storage.insert_sensor_data(sender, sensor_data.clone()) {
            Ok(Some(_)) => {}
            Ok(None) => {
                warn!("Received sensor data for yet-to-register peer, discarding");
                return;
            }
            Err(err) => {
                error!("Error while saving sensor data to storage: {}", err);
                return;
            }
        }

        if let Err(err) = self.webserver_tx.send(WebserverMessage::SensorData {
            node: sender.to_base58(),
            data: sensor_data,
        }) {
            debug!(
                "Error while sending sensor data to web server (most likely OK): {}",
                err
            );
        };
    }

    async fn handle_swarm_event(&mut self, ev: SwarmOutEvent) {
        match ev {
            SwarmOutEvent::Broadcast(ReceivedBroadcast {
                sender,
                broadcast_type,
            }) => {
                let sender_name = match self.storage.peer_name(sender) {
                    Ok(possible_name) => {
                        if let Some(name) = possible_name {
                            name
                        } else {
                            format!("<unregistered peer {}>", sender.to_base58())
                        }
                    }
                    Err(err) => {
                        error!("Error while fetching swarm event: {}", err);
                        format!("<failed to get peer name of peer {}>", sender.to_base58())
                    }
                };
                match broadcast_type {
                    DiotdBroadcast::Identity(peer_data) => {
                        info!(
                            "Updating peer identity: {} --> {:?}",
                            sender_name, peer_data
                        );
                        if let Err(err) = self.storage.insert_peer_data(sender, peer_data.clone()) {
                            error!("Error while updating peer identity: {}", err);
                        }
                        if let Err(err) = self.webserver_tx.send(WebserverMessage::PeerIdentity {
                            node: sender.to_base58(),
                            data: peer_data,
                        }) {
                            debug!("Error while sending identity data to web server (most likely OK): {}", err);
                        };
                    }
                    DiotdBroadcast::SensorData(sensor_data) => {
                        info!("Sensor data received!");
                        info!("  Node: {}", sender_name);
                        info!("  Device: {}", sensor_data.device);
                        info!("  Sensor name: {}", sensor_data.sensor_name);
                        info!("  Value: {:?}", sensor_data.value);

                        self.handle_remote_sensor_data(sender, sensor_data).await;
                    }
                }
            }
            SwarmOutEvent::ActuatorRequest { data, channel } => {
                info!("Actuator request received: {:?}", data);
                self.supervisor.actuate_device_remote(data, channel);
            }
            SwarmOutEvent::ActuatorResponse { id, response } => {
                info!(
                    "Actuator response received for request {:?} received: {:?}",
                    id, response
                );
            }
        }
    }
}
