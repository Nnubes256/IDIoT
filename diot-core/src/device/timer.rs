use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{ConfigurableHardwareDevice, HardwareDevice, Measurement};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimerConfig {
    tick_every_ms: u128,
}

pub struct Timer {
    config: TimerConfig,
    last_tick: Instant,
}

impl HardwareDevice for Timer {
    fn sense(&mut self, sensors: &mut super::SensorData) -> Result<()> {
        if self.last_tick.elapsed().as_millis() > self.config.tick_every_ms {
            sensors.write("tick", Measurement::Signal);
            self.last_tick = Instant::now();
        }

        Ok(())
    }
}

impl ConfigurableHardwareDevice for Timer {
    type Config = TimerConfig;

    fn init(config: Self::Config) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            config,
            last_tick: Instant::now(),
        })
    }
}
