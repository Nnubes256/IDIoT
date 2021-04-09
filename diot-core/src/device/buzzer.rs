use std::{thread, time::Duration};

use anyhow::{Context, Result};
use rppal::gpio::{Gpio, OutputPin};
use serde::{Deserialize, Serialize};

use super::{ActuationResult, ActuatorValue, ConfigurableHardwareDevice, HardwareDevice};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuzzerConfig {
    pin: u8,
}

pub struct Buzzer {
    config: BuzzerConfig,
    pin: OutputPin,
}

impl HardwareDevice for Buzzer {
    fn actuate(&mut self, request: &super::ActuationRequestData) -> ActuationResult {
        match *request.data() {
            ActuatorValue::Signal => {
                // Beep for 1 second
                self.pin.set_high();
                thread::sleep(Duration::from_millis(1000));
                self.pin.set_low();
                ActuationResult::Success
            }
            ActuatorValue::Unsigned(val) => {
                // Beep for N <= 5 seconds
                if val > 0 {
                    if val <= 5 {
                        self.pin.set_high();
                        thread::sleep(Duration::from_secs(val));
                        self.pin.set_low();
                        ActuationResult::Success
                    } else {
                        ActuationResult::BadRequest {
                            reason: format!(
                                "Beep time too long, expected <= 5 seconds, found {} seconds",
                                val
                            ),
                        }
                    }
                } else {
                    ActuationResult::BadRequest {
                        reason: "Zero beep time".to_string(),
                    }
                }
            }
            ActuatorValue::Double(val) => {
                // Beep for N <= 5 seconds
                if val > 0.0 {
                    if val <= 5.0 {
                        self.pin.set_high();
                        thread::sleep(Duration::from_secs_f64(val));
                        self.pin.set_low();
                        ActuationResult::Success
                    } else {
                        ActuationResult::BadRequest {
                            reason: format!(
                                "Beep time too long, expected <= 5 seconds, found {} seconds",
                                val
                            ),
                        }
                    }
                } else {
                    ActuationResult::BadRequest {
                        reason: format!("Zero or negative beep time: {} seconds", val),
                    }
                }
            }
            ActuatorValue::Signed(val) => {
                // Beep for N <= 5 seconds
                if val > 0 {
                    if val <= 5 {
                        self.pin.set_high();
                        // Cast is safe because 0 <= N <= 5
                        thread::sleep(Duration::from_secs(val.abs() as u64));
                        self.pin.set_low();
                        ActuationResult::Success
                    } else {
                        ActuationResult::BadRequest {
                            reason: format!(
                                "Beep time too long, expected <= 5 seconds, found {} seconds",
                                val
                            ),
                        }
                    }
                } else {
                    ActuationResult::BadRequest {
                        reason: format!("Zero or negative beep time: {} seconds", val),
                    }
                }
            }
            ActuatorValue::String(_) => ActuationResult::BadRequest {
                reason: "Strings are unsupported".to_string(),
            },
        }
    }
}

impl ConfigurableHardwareDevice for Buzzer {
    type Config = BuzzerConfig;

    fn init(config: Self::Config) -> Result<Self>
    where
        Self: Sized,
    {
        let pin_num = config.pin;
        let gpio = Gpio::new().context("Couldn't get GPIO")?;
        let pin = gpio
            .get(pin_num)
            .with_context(|| format!("Couldn't obtain GPIO pin {}", pin_num))?
            .into_output();
        Ok(Self { config, pin })
    }
}
