//! Sensor lib for defining base read/write operations
//! for the soil sensor(s) used in plant-minder builds.
//!
//! Currently only supports the ATSAMD10 chip as the soil sensor,
//! with potential for support for other chips (TBD)
//!
//! The intended opertional mode of these sensors is to be operated
//! by a remote, semi-autonomous microcontroller that controls the
//! sensor. The microcontroller uses the thread protocol as
//! the transport layer, in order to report to a remote RPI
//! the sensor readings.
//!
//! The impl for this config is in the pmindp-esp32-thread package in
//! this workspace, and currently only supports esp32-c6
//! and esp32-h2 dev boards.

#![cfg_attr(not(feature = "std"), no_std)]

//#[cfg(feature = "async")]
//use embedded_hal_async::i2c::I2c;
//#[cfg(not(feature = "async"))]
use embedded_hal::i2c::I2c;

use core::ops::FnOnce;
use core::result::Result;
use core::result::Result::Ok;

pub enum SoilSensorError {
    I2cReadError,
    I2cWriteError,
}

/// Seesaw I2C Soil Sensor
pub struct ATSAMD10<I2C: I2c> {
    pub temp_delay: u32,
    pub moisture_delay: u32,
    pub address: u8,
    pub i2c: I2C,
}

pub trait SensorReading {}

impl SensorReading for ATSAMD10SensorReading {}

#[derive(Debug, Clone, Copy)]
pub struct ATSAMD10SensorReading {
    pub moisture: u16,
    pub temperature: f32,
}

// NOTE: can use something like this later
// when ready to implement different soil
// sensor types, for now only using ATSAMD10
//pub struct Sensor<B: BaseSoilSensor> {
//    pub sensor: B,
//}

impl<I2C: I2c> ATSAMD10<I2C> {
    const ATSAMD10_READ_MOISTURE: [u8; 2] = [0x0f, 0x10];
    const ATSAMD10_READ_TEMP: [u8; 2] = [0x00, 0x04];

    pub fn new(i2c: I2C, address: u8, temp_delay: u32, moisture_delay: u32) -> Self {
        Self {
            i2c,
            temp_delay,
            moisture_delay,
            address,
        }
    }

    pub fn read_sensor(
        &mut self,
        r_buffer: &mut [u8],
        w_buffer: &[u8],
        delay: u32,
        f: impl FnOnce(u32) -> (),
    ) -> Result<(), SoilSensorError> {
        self.i2c
            .write(self.address, w_buffer)
            .map_err(|_| SoilSensorError::I2cWriteError)?;
        f(delay);
        self.i2c
            .read(self.address, r_buffer)
            .map_err(|_| SoilSensorError::I2cReadError)?;
        Ok(())
    }

    pub fn moisture(&mut self, f: impl FnOnce(u32)) -> Result<u16, SoilSensorError> {
        let mut buffer = [0; 2];
        self.read_sensor(
            &mut buffer,
            &Self::ATSAMD10_READ_MOISTURE,
            self.moisture_delay,
            f,
        )?;
        Ok(u16::from_be_bytes(buffer))
    }

    pub fn temperature(&mut self, f: impl FnOnce(u32)) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_TEMP, self.temp_delay, f)?;
        let raw = i32::from_be_bytes(buffer) as f32;
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }
}
