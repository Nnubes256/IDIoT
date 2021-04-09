use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{ConfigurableHardwareDevice, HardwareDevice, Measurement};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dht11Config {
    pin: i32,
}

pub struct Dht11 {
    config: Dht11Config,
}

impl HardwareDevice for Dht11 {
    fn sense(&mut self, sensors: &mut super::SensorData) -> Result<()> {
        let mut humidity = 0.0;
        let mut temperature = 0.0;
        let return_value = unsafe {
            adafruit_dht11_sys::pi_2_dht_read(11, self.config.pin, &mut humidity, &mut temperature)
        };
        match return_value {
            0 => {
                sensors.write("temperature", Measurement::Double(temperature.into()));
                sensors.write("humidity", Measurement::Double(humidity.into()));
            }
            -1 => {
                // Device not available yet
            }
            -2 => {
                // Checksum error
            }
            err => {}
        };
        Ok(())
    }
}

impl ConfigurableHardwareDevice for Dht11 {
    type Config = Dht11Config;

    fn init(config: Self::Config) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self { config })
    }
}
