#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::items_after_statements)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_const_for_fn,
    clippy::inefficient_to_string,
    clippy::multiple_crate_versions,
    clippy::redundant_pub_crate,
    clippy::use_self
)]

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate async_trait;

mod control;
mod hardware;
mod store;
mod swarm;
mod system;
mod web;

use anyhow::{Context, Result};

use libp2p::{
    identity::{ed25519, Keypair},
    pnet::PreSharedKey,
};
use rand::Rng;
use rand::{prelude::StdRng, SeedableRng};
use system::{PeerSecrets, System, SystemConfig};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
};

fn generate_psk() -> Result<PreSharedKey> {
    let mut rng = StdRng::from_entropy();
    let mut out = [0_u8; 32];
    rng.try_fill(&mut out)
        .context("Failed to fill PSK buffer with random data")?;
    Ok(PreSharedKey::new(out))
}

fn generate_keypair() -> ed25519::Keypair {
    let new_keypair = Keypair::generate_ed25519();
    if let Keypair::Ed25519(ed25519_keypair) = new_keypair {
        ed25519_keypair
    } else {
        unreachable!()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut config: SystemConfig = {
        let file = File::open("config.json")
            .await
            .context("Couldn't open config file")?;
        let mut rdr = BufReader::new(file);
        let mut buf = Vec::new();
        rdr.read_to_end(&mut buf)
            .await
            .context("Failed to read config file")?;

        serde_json::from_slice(&buf).context("Couldn't parse config file")?
    };

    if config.secrets.is_none() {
        info!("Generating new peer keypair and pre-shared key");
        let psk = generate_psk().context("Couldn't generate pre-shared key")?;
        let keypair = generate_keypair();
        info!("Saving new secrets to config file");
        let mut file = OpenOptions::new()
            .write(true)
            .open("config.json")
            .await
            .context("Couldn't open config file for writing")?;
        config.secrets = Some(PeerSecrets { psk, keypair });
        let config_json =
            serde_json::to_vec_pretty(&config).context("Couldn't re-serialize the config file")?;
        file.write(&config_json)
            .await
            .context("Couldn't write back to the config file")?;
    }

    let mut system = System::from_config(config)
        .await
        .context("Failed to initialize system")?;

    system
        .launch()
        .await
        .context("Unrecoverable runtime error on system")?;

    Ok(())
}
