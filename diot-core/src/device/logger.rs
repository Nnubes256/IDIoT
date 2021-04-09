use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{ActuationResult, ActuatorValue, ConfigurableHardwareDevice, HardwareDevice};

const SIGNAL_DEFAULT: &str = "Received signal!";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoggerConfig {
    prefix: Option<String>,
    suffix: Option<String>,
    signal: Option<String>,
}

pub struct Logger {
    prefix: String,
    suffix: String,
    signal: String,
}

impl HardwareDevice for Logger {
    fn actuate(&mut self, request: &super::ActuationRequestData) -> ActuationResult {
        match request.data() {
            ActuatorValue::Signal => {
                info!("{}: {}", request.actuator_name(), self.signal);
            }
            val => {
                info!(
                    "{}: {}{}{}",
                    request.actuator_name(),
                    self.prefix,
                    val,
                    self.suffix
                );
            }
        };

        ActuationResult::Success
    }
}

impl ConfigurableHardwareDevice for Logger {
    type Config = LoggerConfig;

    fn init(config: Self::Config) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            prefix: config.prefix.unwrap_or_else(String::new),
            suffix: config.suffix.unwrap_or_else(String::new),
            signal: config
                .signal
                .unwrap_or_else(|| String::from(SIGNAL_DEFAULT)),
        })
    }
}
