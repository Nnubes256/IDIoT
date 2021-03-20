use core::time::Duration;
use std::{
    alloc::System,
    collections::{hash_map::Drain, HashMap, VecDeque},
    marker::PhantomData,
};

use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use zerocopy::{AsBytes, FromBytes, Unaligned};

pub mod dht11;

/*


/// An accesor for a given sensor
///
/// This structure is created through [`SensorVisit::sensor()`], and bound to the [`SensorVisit`] instance
/// that created it (meaning the compiler will force you to drop this accesor before the current polling cycle),
/// and allows to publish a single value of a given type to the sensor specified to it.
pub struct Sensor<'dev, T> {
    visit: &'dev mut SensorVisit,
    name: String,
    _t: PhantomData<T>,
}

impl<'dev, T> Sensor<'dev, T> {
    fn new(visit: &'dev mut SensorVisit, name: String) -> Sensor<'dev, T> {
        Self {
            visit,
            name,
            _t: PhantomData::<T>,
        }
    }
}

#[derive(Debug, Error)]
pub enum SensorAccessError {
    #[error("Tried to access sensor {} with intent of writing values of type {:?}, but {:?} was expected", name, current, requested)]
    MeasurementKindMismatch {
        name: String,
        current: MeasurementKind,
        requested: MeasurementKind,
    },
    #[error("Sensor {} was already accessed on the current polling cycle", name)]
    SensorAlreadyAccessed { name: String },
}

/// Sensor publishing endpoint for devices.
///
/// This structure represents the device's main interface with the wider network.
/// It gets created and provided to the device's [`sense()`](Device::sense) callback every once in a set interval.
#[derive(Default)]
pub struct SensorVisit {
    pub(self) sensor_state: HashMap<String, Measurement>,
    sensor_measure_kinds: HashMap<String, MeasurementKind>,
}

impl SensorVisit {
    pub fn sensor<T, S>(&mut self, name: S) -> Result<Sensor<'_, T>, SensorAccessError>
    where
        Self: SensorAccesor<T>,
        S: AsRef<str>,
    {
        self.sensor_impl(name)
    }
}

macro_rules! impl_measurements {
    {$($vartype:ty => $variant:ident),*} => {
        $(
            impl<'dev> Sensor<'dev, $vartype> {
                /// Publish the given measurement for this sensor
                pub fn publish(self, value: $vartype) {
                    self.visit.sensor_state.insert(self.name, Measurement::$variant(value));
                }
            }
        )*

        /// Provides access to instantiate various type-casted [`Sensor`]s.
        ///
        /// Users commonly don't need to import this trait; instead, use [`SensorVisit::sensor()`] directly.
        pub trait SensorAccesor<T> {
            /// Instantiates a [`Sensor`] of type `T` for the given sensor name.
            fn sensor_impl<S: AsRef<str>>(&mut self, name: S) -> Result<Sensor<'_, T>, SensorAccessError>;
        }

        $(
            impl SensorAccesor<$vartype> for SensorVisit {
                fn sensor_impl<S: AsRef<str>>(&mut self, name: S) -> Result<Sensor<'_, $vartype>, SensorAccessError> {
                    use std::collections::hash_map::Entry;
                    use self::SensorAccessError::*;
                    let name = name.as_ref().to_owned();

                    // First, ensure the user is always writing a consistent type to a given sensor.
                    match self.sensor_measure_kinds.entry(name.clone()) {
                        Entry::Occupied(kind) => {
                            let kind = *kind.get();
                            if kind != MeasurementKind::$variant {
                                return Err(MeasurementKindMismatch { name: name.clone(), current: MeasurementKind::$variant, requested: kind } )
                            }
                        },
                        Entry::Vacant(v) => {
                            v.insert(MeasurementKind::$variant);
                        },
                    };

                    // Then, ensure the user has only written to this sensor once in this cycle.
                    if self.sensor_state.contains_key(&name) {
                        Err(SensorAlreadyAccessed { name: name.clone() })
                    } else {
                        Ok(Sensor::<$vartype>::new(self, name))
                    }
                }
            }
        )*
    };
}

impl_measurements! {
    u8 => U8,
    u16 => U16,
    u32 => U32,
    u64 => U64,
    i8 => I8,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    f64 => Double,
    String => String
}

/// A device, composed of various sensors and actuators in any arrangement
pub trait Device2 {
    /// Initializes the device and returns an instance of this device
    fn init() -> Self
    where
        Self: Sized;

    /// Called when the system wants to poll this device for sensory output.
    ///
    /// Typically, here you will implement the logic to read sensor data.
    fn sense(&mut self, visit: &mut SensorVisit) -> Result<()>;

    /// Called when we want to reset the device, typically on shutdown
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}*/

#[derive(Debug, EnumKind, Serialize, Deserialize)]
#[enum_kind(MeasurementKind)]
pub enum Measurement {
    Unsigned(u64),
    Signed(i64),
    Double(f64),
    String(String),
}

pub enum ActuatorValue {
    Signal,
    Unsigned(u64),
    Signed(i64),
    Double(f64),
    String(String),
}

#[derive(Debug, Error)]
pub enum ActuationError {
    #[error("Actuation request was ignored: {}", reason)]
    Ignored { reason: String },
    #[error("Bad request: {:?}", _0)]
    BadRequest(anyhow::Error),
    #[error("Actuator error: {:?}", _0)]
    ActuatorError(anyhow::Error),
}

pub trait ResponseChannel {
    fn send(self: Box<Self>, response: Result<(), ActuationError>) -> Result<()>;
}

pub struct ActuationRequest {
    actuator_name: String,
    data: ActuatorValue,
    out: Box<dyn ResponseChannel>,
}

impl ActuationRequest {
    pub fn actuator_name(&self) -> &str {
        &self.actuator_name
    }

    pub fn data(&self) -> &ActuatorValue {
        &self.data
    }

    pub fn answer(self, response: Result<(), ActuationError>) -> Result<()> {
        self.out.send(response)
    }
}

pub trait SystemBridge {
    fn write_sensor_data(&mut self, name: String, value: Measurement);
    fn actuator_request_next(&mut self) -> Result<Option<ActuationRequest>>;
}

pub struct SensorData<'sys> {
    system: &'sys mut dyn SystemBridge,
}

impl<'sys> SensorData<'sys> {
    pub fn new(system: &'sys mut dyn SystemBridge) -> Self {
        Self { system }
    }

    pub fn write<S: Into<String>>(&mut self, name: S, value: Measurement) {
        self.system.write_sensor_data(name.into(), value);
    }
}

pub struct ActuatorRequests<'sys> {
    system: &'sys mut dyn SystemBridge,
}

impl<'sys> ActuatorRequests<'sys> {
    pub fn new(system: &'sys mut dyn SystemBridge) -> Self {
        Self { system }
    }
}

impl<'sys> Iterator for ActuatorRequests<'sys> {
    type Item = Result<ActuationRequest>;

    fn next(&mut self) -> Option<Self::Item> {
        self.system.actuator_request_next().transpose()
    }
}

/// A device, composed of various sensors and actuators in any arrangement
pub trait Device {
    /// Initializes the device and returns an instance of this device.
    fn init() -> Self
    where
        Self: Sized;

    /// Called when the system wants to poll this device for sensory output.
    ///
    /// Typically, here you will implement the logic to read sensor data.
    fn sense(&mut self, sensors: &mut SensorData) -> Result<()>;

    /// Called when the system has requests to actuate this device.
    fn actuate(&mut self, requests: &mut ActuatorRequests) -> Result<()>;

    /// Called when we want to reset the device, typically on shutdown.
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}
