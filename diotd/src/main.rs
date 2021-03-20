#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(missing_docs)]
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

mod store;
mod swarm;
mod system;

use anyhow::{Context, Result};

use libp2p::pnet::PreSharedKey;
use rand::Rng;
use rand::{prelude::StdRng, SeedableRng};
use system::{System, SystemConfig};
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

    if config.psk.is_none() {
        info!("Generating new pre-shared key");
        let psk = generate_psk().context("Couldn't generate pre-shared key")?;
        info!("Saving new pre-shared key to config file");
        let mut file = OpenOptions::new()
            .write(true)
            .open("config.json")
            .await
            .context("Couldn't open config file for writing")?;
        config.psk = Some(format!("{}", psk));
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
