//! Sensor lib for defining base read/write operations
//! for the sensor(s) used in plant-minder builds.
//!
//! The intended opertional mode of these sensors is to be operated
//! by a remote, semi-autonomous microcontroller that controls the
//! sensor. The microcontroller uses the thread protocol as
//! the transport layer, in order to report to a remote RPI
//! the sensor readings.
//!
//! The impl for these is in the pmindp-esp32-thread package in
//! this workspace, and currently only supports esp32-c6
//! and esp32-h2 dev boards.

#![cfg_attr(not(feature = "std"), no_std)]

use serde::{Deserialize, Serialize};

/// [`PlantConfig`] struct is used at compile time by
/// esp32 nodes, to report to the RPI what plants they
/// are currently associated with
#[toml_cfg::toml_config]
pub struct PlantConfig {
    #[default(666)]
    pot_num: u32,
    #[default("SirPots")]
    name: &'static str,
}

/// System must have at a bare minimum soil sensor, all other
/// sensors are optional
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct SensorReading {
    pub soil: Soil,
    pub light: Option<Light>,
    pub gas: Option<Gas>,
    #[serde(default)]
    pub ts: i64,
}

pub const MAX_SENSORS: usize = 5;
pub const SOIL_IDX: usize = 0;
pub const LIGHT_IDX_1: usize = 1;
pub const HUM_IDX: usize = 2;
pub const LIGHT_IDX_2: usize = 3;
pub const OTHER_IDX: usize = 4;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Soil {
    /// Soil Moisture
    pub moisture: u16,
    /// Optional temperature reading (some sensors may not have this)
    #[serde(default)]
    pub temp: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Light {
    /// Full Spectrum light reading
    pub fs: u16,
    /// Lux reading
    pub lux: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Gas {
    /// Temparature
    pub temp: f32,
    /// Pressure
    pub p: f32,
    /// Humidity
    pub h: f32,
    /// Gas resistance
    pub gas: u32,
}

/// [`SensorPlatform`] trait defines the sensor read operation for the platform,
/// which is configured to hold a vec of dynamic [`Sensor`] objects to
/// allow support for different sensor types
pub trait SensorPlatform {
    fn sensor_read(&self, buff: &mut [u8]) -> Result<SensorReading, PlatformSensorError>;
}

/// [`Sensor`] trait defines the base sensor read operation, to allow support for
/// different sensor types. For each sensor type that a given platform
/// can support, this operation should pull all possible data fields (e.g. some
/// sensors may only support pulling moisture, others moisture + temp, others
/// only report lumens/lux etc). Relies on [`MoistureSensor`], [`TempSensor`],
/// [`LightLumenSensor`], and [`Lux Sensor`] to provide support for
/// device-specific data read ops
pub trait Sensor {
    fn read(&mut self, buffer: &mut [u8], index: usize) -> Result<usize, PlatformSensorError>;
}

/// allows device-specific impls of moisture-specific sensor functionality
pub trait MoistureSensor {
    fn moisture(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, SoilSensorError>;
}

/// allows device-specific impls of temp-specific sensor functionality
pub trait TempSensor {
    fn temp(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, SoilSensorError>;
}

/// allows device-specific impls of lumens-specific sensor functionality
pub trait LightLumenSensor {
    fn luminosity(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, LightSensorError>;
}

/// allows device-specific impls of lux-specific sensor functionality
pub trait LightLuxSensor {
    fn lux(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, LightSensorError>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum I2cError {
    I2cReadError,
    I2cWriteError,
    I2cWriteReadError,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LightSensorError {
    I2cError(I2cError),
    SetupError,
    SensorError,
    SignalOverflow,
}

impl From<I2cError> for LightSensorError {
    fn from(e: I2cError) -> Self {
        LightSensorError::I2cError(e)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SoilSensorError {
    I2cReadError,
    I2cWriteError,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PlatformSensorError {
    SoilSensorError(SoilSensorError),
    LightSensorError(LightSensorError),
    SensorSetup,
    Other,
}

impl From<SoilSensorError> for PlatformSensorError {
    fn from(e: SoilSensorError) -> Self {
        PlatformSensorError::SoilSensorError(e)
    }
}

impl From<LightSensorError> for PlatformSensorError {
    fn from(e: LightSensorError) -> Self {
        PlatformSensorError::LightSensorError(e)
    }
}
