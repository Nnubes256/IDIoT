use std::{fmt::Display, marker::PhantomData};

use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeviceInitError {
    #[error("Unknown device type: {}", device_type)]
    UnknownDeviceType { device_type: String },
    #[error("Invalid config for device")]
    InvalidConfig(#[from] serde_json::Error),
    #[error("Device returned error while initializing")]
    DeviceError(#[from] anyhow::Error),
}

macro_rules! register_devices {
    {$($(#[$attr:meta])* $name:ident as $stringified:expr => $module:ident;)*} => {
        $(
            $(#[$attr])*
            pub mod $module;
        )*

        #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
        pub enum HardwareDeviceType {
            $(
                #[serde(rename = $stringified)]
                $(#[$attr])*
                $name
            ),*
        }

        use std::fmt::Debug;

        impl Display for HardwareDeviceType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        $(#[$attr])*
                        HardwareDeviceType::$name => write!(f, $stringified)
                    ),*
                }
            }
        }

        pub fn initialize_device(dev_type: HardwareDeviceType, config: serde_json::Value) -> Result<Box<dyn HardwareDevice>> {
            match dev_type {
                $(
                    $(#[$attr])*
                    HardwareDeviceType::$name => {
                        let conf_parsed: <$module::$name as ConfigurableHardwareDevice>::Config = serde_json::from_value(config)?;
                        let dev = $module::$name::init(conf_parsed)?;
                        Ok(Box::new(dev))
                    }
                ),*
            }
        }
    };
}

register_devices! {
    Timer as "timer" => timer;
    Logger as "logger" => logger;
    #[cfg(target_arch = "arm")]
    Dht11 as "dht11" => dht11;
    #[cfg(target_arch = "arm")]
    Buzzer as "buzzer" => buzzer;
}

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ActuationResult {
    Success,
    Ignored,
    NoResponse,
    BadRequest {
        reason: String,
    },
    ActuatorError {
        error_code: i64,
        error_description: String,
    },
}

#[derive(Clone, Debug, PartialEq, EnumKind, Serialize, Deserialize)]
#[enum_kind(MeasurementKind)]
#[repr(C)]
#[serde(rename_all = "snake_case")]
pub enum Measurement {
    Signal,
    Integer(i64),
    Double(f64),
    String(String),
}

impl Measurement {
    pub fn kind(&self) -> MeasurementKind {
        match self {
            Measurement::Signal => MeasurementKind::Signal,
            Measurement::Integer(_) => MeasurementKind::Integer,
            Measurement::Double(_) => MeasurementKind::Double,
            Measurement::String(_) => MeasurementKind::String,
        }
    }

    pub fn gt(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Measurement::Integer(a), Measurement::Integer(b)) => Some(a.gt(b)),
            (Measurement::Double(a), Measurement::Double(b)) => Some(a.gt(b)),
            _ => None,
        }
    }

    pub fn lt(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Measurement::Integer(a), Measurement::Integer(b)) => Some(a.lt(b)),
            (Measurement::Double(a), Measurement::Double(b)) => Some(a.lt(b)),
            _ => None,
        }
    }

    pub fn geq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Measurement::Integer(a), Measurement::Integer(b)) => Some(a.ge(b)),
            (Measurement::Double(a), Measurement::Double(b)) => Some(a.ge(b)),
            _ => None,
        }
    }

    pub fn leq(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Measurement::Integer(a), Measurement::Integer(b)) => Some(a.le(b)),
            (Measurement::Double(a), Measurement::Double(b)) => Some(a.le(b)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActuatorValue {
    Signal,
    Unsigned(u64),
    Signed(i64),
    Double(f64),
    String(String),
}

impl Display for ActuatorValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ActuatorValue::Signal => {
                write!(f, "<signal>")
            }
            ActuatorValue::Unsigned(val) => {
                write!(f, "{}", val)
            }
            ActuatorValue::Signed(val) => {
                write!(f, "{}", val)
            }
            ActuatorValue::Double(val) => {
                write!(f, "{}", val)
            }
            ActuatorValue::String(ref val) => {
                write!(f, "{}", val)
            }
        }
    }
}

pub trait ActuatorResponseChannel: Send {
    fn send(self, response: ActuationResult);
}

pub struct ActuationRequest<O: ActuatorResponseChannel> {
    data: ActuationRequestData,
    out_chan: O,
}

impl<O: ActuatorResponseChannel> ActuationRequest<O> {
    pub fn new(data: ActuationRequestData, out_chan: O) -> Self {
        Self { data, out_chan }
    }

    pub fn data(&self) -> &ActuationRequestData {
        &self.data
    }

    pub fn send_answer(self, response: ActuationResult) {
        self.out_chan.send(response);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActuationRequestData {
    actuator_name: String,
    data: ActuatorValue,
}

impl ActuationRequestData {
    pub fn new(actuator_name: String, data: ActuatorValue) -> Self {
        Self {
            actuator_name,
            data,
        }
    }

    pub fn actuator_name(&self) -> &str {
        &self.actuator_name
    }

    pub fn data(&self) -> &ActuatorValue {
        &self.data
    }
}

trait SystemBridgeInner {
    fn write_sensor_data_inner(&mut self, name: String, value: Measurement);
}

pub trait SystemBridge {
    type ActuatorRequestOutChannel: ActuatorResponseChannel;

    fn write_sensor_data(&mut self, name: String, value: Measurement);
    fn actuator_request_next(
        &mut self,
    ) -> Result<Option<ActuationRequest<Self::ActuatorRequestOutChannel>>>;

    fn sensor_collector(&'_ mut self) -> SensorData<'_>
    where
        Self: Sized,
    {
        SensorData::new(self)
    }
    fn actuator_provider(&'_ mut self) -> ActuatorRequests<'_, Self::ActuatorRequestOutChannel>
    where
        Self: Sized,
    {
        ActuatorRequests::new(self)
    }
}

impl<T, U> SystemBridgeInner for T
where
    U: ActuatorResponseChannel,
    T: SystemBridge<ActuatorRequestOutChannel = U>,
{
    fn write_sensor_data_inner(&mut self, name: String, value: Measurement) {
        self.write_sensor_data(name, value);
    }
}

pub struct SensorData<'sys> {
    system: &'sys mut dyn SystemBridgeInner,
}

impl<'sys> SensorData<'sys> {
    fn new(system: &'sys mut dyn SystemBridgeInner) -> Self {
        Self { system }
    }

    pub fn write<S: Into<String>>(&mut self, name: S, value: Measurement) {
        self.system.write_sensor_data_inner(name.into(), value);
    }
}

pub struct ActuatorRequests<'sys, O: ActuatorResponseChannel> {
    system: &'sys mut dyn SystemBridge<ActuatorRequestOutChannel = O>,
    _marker: PhantomData<O>,
}

impl<'sys, O: ActuatorResponseChannel> ActuatorRequests<'sys, O> {
    pub fn new(system: &'sys mut dyn SystemBridge<ActuatorRequestOutChannel = O>) -> Self {
        Self {
            system,
            _marker: PhantomData,
        }
    }
}

impl<'sys, O: ActuatorResponseChannel> Iterator for ActuatorRequests<'sys, O> {
    type Item = Result<ActuationRequest<O>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.system.actuator_request_next().transpose()
    }
}

/// A device, composed of various sensors and actuators in any arrangement
pub trait HardwareDevice: Send {
    /// Called when the system wants to poll this device for sensory output.
    ///
    /// Typically, here you will implement the logic to read sensor data.
    fn sense(&mut self, _sensors: &mut SensorData) -> Result<()> {
        Ok(())
    }

    /// Called when the system has a request to actuate this device.
    fn actuate(&mut self, _request: &ActuationRequestData) -> ActuationResult {
        ActuationResult::NoResponse
    }

    /// Called when we want to reset the device, typically on shutdown.
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}

pub trait ConfigurableHardwareDevice: HardwareDevice {
    /// Configuration data for the device
    type Config: DeserializeOwned;

    /// Initializes the device and returns an instance of this device.
    fn init(config: Self::Config) -> Result<Self>
    where
        Self: Sized;
}
