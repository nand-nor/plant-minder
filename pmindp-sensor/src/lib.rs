//! Sensor lib for defining base read/write operations
//! for the soil sensor(s) used in plant-minder builds.
//!
//! Currently only supports the ATSAMD10 chip as the soil sensor,
//! with potential for support for other chips (TBD)
//!
//! Two build configurations for this sensor exist:
//!
//! 1. remote sensor builds, where the sensor is operated
//! by a remote, autonomous microcontroller that controls the
//! sensor. The microcontroller uses the thread protocol as
//! the transport layer, in order to report to a remote RPI
//! the sensor readings which will be used for rendering on the pi
//! via the plant-minder front end.
//! The impl for this config is in the pmindp-thread package in
//! this workspace, and currently only supports esp32-c6
//! and esp32-h2 dev boards.
//!
//! 2. Local sensor builds, where the sensor is operated locally
//! by the RPI itself. Currently, locally run/controlled
//! sensor builds are only implemented for multiple sensors, where
//! sensor control is done on the pi via a TCA9548A i2c expander
//! (due to inability to assign each sensor a unique address)
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "async_i2cmux")]
use linux_embedded_hal::i2cdev::linux::LinuxI2CError;
#[cfg(feature = "std")]
use thiserror::Error;
#[cfg(feature = "async_i2cmux")]
use xca9548a::Error as I2cMuxError;

//#[cfg(feature = "async_i2cmux")]
//use embedded_hal_async::i2c::I2c;

//#[cfg(not(feature = "async_i2cmux"))]
use embedded_hal::i2c::I2c;

#[cfg(feature = "std")]
#[derive(Error, Debug)]
pub enum SoilSensorError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Read Error")]
    I2cReadError,
    #[error("Write Error")]
    I2cWriteError,
    #[cfg(feature = "async_i2cmux")]
    #[error("Mux Error")]
    I2cMuxError,
    #[error("Linux i2c error")]
    LinuxError(#[from] LinuxI2CError),
}

#[cfg(feature = "async_i2cmux")]
impl<E> From<I2cMuxError<E>> for SoilSensorError {
    fn from(_error: I2cMuxError<E>) -> SoilSensorError {
        // todo logging
        SoilSensorError::I2cMuxError
    }
}

#[cfg(any(not(feature = "std"), not(feature = "async_i2cmux")))]
use core::ops::FnOnce;
#[cfg(any(not(feature = "std"), not(feature = "async_i2cmux")))]
use core::result::Result;

#[cfg(any(not(feature = "std"), not(feature = "async_i2cmux")))]
use core::result::Result::Ok;

#[cfg(any(not(feature = "std"), not(feature = "async_i2cmux")))]
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

#[cfg(feature = "async_i2cmux")]
unsafe impl<I2C: I2c> Send for ATSAMD10<I2C> {}
#[cfg(feature = "async_i2cmux")]
unsafe impl<I2C: I2c> Sync for ATSAMD10<I2C> {}

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

    #[cfg(feature = "async_i2cmux")]
    pub async fn read_sensor(
        &mut self,
        r_buffer: &mut [u8],
        w_buffer: &[u8],
        delay: u32,
    ) -> Result<(), SoilSensorError> {
        self.i2c
            .write(self.address, w_buffer)
            //    .await
            .map_err(|_| SoilSensorError::I2cWriteError)?;

        // from https://github.com/adafruit/Adafruit_Seesaw/blob/master/Adafruit_seesaw.cpp#L952
        tokio::time::sleep(tokio::time::Duration::from_micros(delay.into())).await;

        self.i2c
            .read(self.address, r_buffer)
            //    .await
            .map_err(|_| SoilSensorError::I2cReadError)?;
        Ok(())
    }
    #[cfg(feature = "async_i2cmux")]
    pub async fn moisture(&mut self) -> Result<u16, SoilSensorError> {
        let mut buffer = [0; 2];
        self.read_sensor(
            &mut buffer,
            &Self::ATSAMD10_READ_MOISTURE,
            self.moisture_delay,
        )
        .await?;
        Ok(u16::from_be_bytes(buffer))
    }

    #[cfg(feature = "async_i2cmux")]
    pub async fn temperature(&mut self) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_TEMP, self.temp_delay)
            .await?;
        let raw = i32::from_be_bytes(buffer) as f32;
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }

    #[cfg(not(feature = "async_i2cmux"))]
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

    #[cfg(not(feature = "async_i2cmux"))]
    pub fn moisture(&mut self, f: impl FnOnce(u32) -> ()) -> Result<u16, SoilSensorError> {
        let mut buffer = [0; 2];
        self.read_sensor(
            &mut buffer,
            &Self::ATSAMD10_READ_MOISTURE,
            self.moisture_delay,
            f,
        )?;
        Ok(u16::from_be_bytes(buffer))
    }

    #[cfg(not(feature = "async_i2cmux"))]
    pub fn temperature(&mut self, f: impl FnOnce(u32) -> ()) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_TEMP, self.temp_delay, f)?;
        let raw = i32::from_be_bytes(buffer) as f32;
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }
}
