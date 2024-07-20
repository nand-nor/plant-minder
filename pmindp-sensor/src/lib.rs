//! Sensor lib for defining base read/write operations
//! for the soil sensor(s) used in plant-minder builds.
//!
//! Currently only supports two sensor types:
//! 1. Seesaw soil sensor (ATSAMD10)
//! 2. Sparkfun soul sensor (Probe Circuit)
//! with potential for support for other chips (TBD)
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

pub enum SoilSensorError {
    I2cReadError,
    I2cWriteError,
}

#[derive(Debug, Clone, Copy)]
pub struct SensorReading {
    pub moisture: u16,
    pub temperature: f32,
}

/// Trait to define base sensor operations for pulling
/// moisture and optional temperature readings 
pub trait SoilSensor {
    type InputParams;
    type MoistureOutput: core::fmt::Debug;
    type TemperatureOutput: core::fmt::Debug;

    fn moisture(&mut self, r: Self::InputParams) -> Result<Self::MoistureOutput, SoilSensorError>;
    fn temperature(
        &mut self,
        r: Self::InputParams,
    ) -> Result<Self::TemperatureOutput, SoilSensorError>;
}
/// Trait that defines the sensor read operation, to allow support for
/// different sensor types
pub trait SoilSensorPlatform {
    type Sensor: SoilSensor;
    fn sensor_read(&self, buff: &mut [u8]) -> Result<(), SoilSensorError>;
}
