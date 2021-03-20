use std::{path::PathBuf, str::FromStr, time::Duration};

use anyhow::{bail, Context, Result};
use libp2p::{
    gossipsub::{error::PublishError, IdentTopic},
    pnet::PreSharedKey,
    Multiaddr, Swarm,
};

use crate::swarm::{setup_swarm, DiodtSwarm, DiotdBroadcast, PeerData};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemConfig {
    pub peer: PeerData,
    pub psk: Option<String>,
    pub database: PathBuf,
}

pub struct System {
    swarm: DiodtSwarm,
    db: sled::Db,
    config: SystemConfig,
}

impl System {
    pub async fn from_config(config: SystemConfig) -> Result<Self> {
        let psk = if let Some(psk_str) = &config.psk {
            PreSharedKey::from_str(psk_str)
                .context("Couldn't parse pre-shared key from config file")?
        } else {
            bail!("Missing pre-shared key (BUG)");
        };

        let db = sled::open(&config.database).context("Couldn't open database")?;

        let swarm: DiodtSwarm = setup_swarm(&db, psk)
            .await
            .context("Couldn't setup swarm")?;

        Ok(Self { config, swarm, db })
    }

    pub async fn launch(&mut self) -> Result<()> {
        let addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
        Swarm::listen_on(&mut self.swarm, addr)
            .context("Swarm was unable to start listening on the network")?;
        self.system_loop().await;
        Ok(())
    }

    async fn broadcast_identity(&mut self) {
        let topic = IdentTopic::new("default");
        let message = bincode::serialize(&DiotdBroadcast::Identity(self.config.peer.clone()))
            .expect("Failed to serialize config?!");
        match self.swarm.gossipsub.publish(topic, message) {
            Ok(id) => debug!("Sent msg with ID: {}", id),
            Err(err) => match err {
                PublishError::InsufficientPeers => {}
                err => error!("Error while sending message: {:?}", err),
            },
        }
    }

    async fn system_loop(&mut self) {
        let mut timer = tokio::time::interval(Duration::from_secs(5));

        let mut listening = false;

        loop {
            tokio::select! {
                _ = timer.tick() => {
                    self.broadcast_identity().await;
                }
                event = self.swarm.next() => {
                    // All events are handled by the `NetworkBehaviourEventProcess`es.
                    // I.e. the `swarm.next()` future drives the `Swarm` without ever
                    // terminating.
                    panic!("Unexpected event: {:?}", event);
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
}
