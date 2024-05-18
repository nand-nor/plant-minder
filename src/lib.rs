pub mod soil_sensor;

use linux_embedded_hal::i2cdev::linux::LinuxI2CError;
use thiserror::Error;

use crate::soil_sensor::SoilSensorError;
use xca9548a::Error as I2cMuxError;

#[derive(Error, Debug)]
pub enum PlantMinderError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),

    #[error("I2c Read Error")]
    I2cReadError,
    #[error("I2c Write Error")]
    I2cWriteError,

    #[error("TCA9548A I2c mux error")]
    I2cMuxError,

    #[error("Linux Embedded Hal i2c error")]
    EmbeddedHal(#[from] LinuxI2CError),

    #[error("Sensor error")]
    SpoilSensorError(#[from] SoilSensorError),
}

impl<E> From<I2cMuxError<E>> for PlantMinderError {
    fn from(_error: I2cMuxError<E>) -> PlantMinderError {
        // todo logging
        PlantMinderError::I2cMuxError
    }
}
