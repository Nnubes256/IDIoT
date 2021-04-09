use std::collections::HashMap;

use anyhow::Result;
use dashmap::DashMap;
use diot_core::device::{HardwareDeviceType, Measurement};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

use crate::{hardware::FullSensorData, swarm::PeerData, system::LocalPeerData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPeerDevice {
    pub device_type: HardwareDeviceType,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePeerDevice {
    pub device_type: HardwareDeviceType,
}

impl From<LocalPeerDevice> for RemotePeerDevice {
    fn from(dev: LocalPeerDevice) -> Self {
        RemotePeerDevice {
            device_type: dev.device_type,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorState {
    pub current_value: Measurement,
}

impl SensorState {
    pub fn new(current_value: Measurement) -> Self {
        Self { current_value }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceState {
    pub device_type: HardwareDeviceType,
    pub sensors: HashMap<String, SensorState>,
}

impl DeviceState {
    pub fn from_device_type(device_type: HardwareDeviceType) -> Self {
        Self {
            device_type,
            sensors: HashMap::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerState {
    pub name: String,
    pub devices: HashMap<String, DeviceState>,
}

impl PeerState {
    fn from_peer_data(data: PeerData) -> Self {
        let mut devices = HashMap::with_capacity(data.devices.len());

        for (device_name, RemotePeerDevice { device_type }) in data.devices {
            devices.insert(device_name, DeviceState::from_device_type(device_type));
        }

        Self {
            name: data.name,
            devices,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct StoredPeerId {
    peer_id: PeerId,
}

impl From<PeerId> for StoredPeerId {
    fn from(peer_id: PeerId) -> Self {
        Self { peer_id }
    }
}

impl Serialize for StoredPeerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.peer_id.to_base58();
        serializer.serialize_str(&s)
    }
}

#[derive(Debug, Default, Serialize)]
pub struct FullSystemState {
    pub peers: DashMap<StoredPeerId, PeerState>,
}

pub struct Storage {
    local_peer_id: PeerId,
    cache: FullSystemState,
}

impl Storage {
    pub fn new(local_peer_id: PeerId, local_peer_data: LocalPeerData) -> Result<Self> {
        let storage = Self {
            local_peer_id,
            cache: FullSystemState::default(),
        };

        storage.insert_peer_data(storage.local_peer_id(), local_peer_data.into())?;

        Ok(storage)
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn peer_name(&self, peer: PeerId) -> Result<Option<String>> {
        let peer = match self.cache.peers.get(&peer.into()) {
            Some(peer) => peer,
            None => return Ok(None),
        };

        Ok(Some(peer.name.clone()))
    }

    #[allow(dead_code)]
    pub fn sensor_data(
        &self,
        peer: PeerId,
        device_name: &str,
        sensor_name: &str,
    ) -> Result<Option<Measurement>> {
        let peer = match self.cache.peers.get(&peer.into()) {
            Some(peer) => peer,
            None => return Ok(None),
        };

        Ok(peer
            .devices
            .get(device_name)
            .and_then(|device_state| device_state.sensors.get(sensor_name))
            .map(|sensor_state| sensor_state.current_value.clone()))
    }

    pub fn full_system_state(&self) -> &FullSystemState {
        &self.cache
    }

    pub fn insert_peer_data(&self, peer: PeerId, peer_data: PeerData) -> Result<()> {
        self.cache
            .peers
            .insert(peer.into(), PeerState::from_peer_data(peer_data));

        Ok(())
    }

    pub fn insert_sensor_data(
        &self,
        peer: PeerId,
        sensor_data: FullSensorData,
    ) -> Result<Option<()>> {
        let mut peer = match self.cache.peers.get_mut(&peer.into()) {
            Some(peer) => peer,
            None => return Ok(None),
        };

        Ok(peer
            .devices
            .get_mut(&sensor_data.device)
            .map(|device_state| {
                device_state
                    .sensors
                    .insert(sensor_data.sensor_name, SensorState::new(sensor_data.value));
            }))
    }
}
