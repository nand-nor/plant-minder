use linux_embedded_hal::I2cdev;
use thiserror::Error;
use tokio::time::{sleep, Duration};
use xca9548a::{I2cSlave, Xca9548a};

use embedded_hal::blocking::i2c::{Read, Write};
use embedded_hal_async::i2c::{
    Error as AsyncHalI2cError, ErrorKind as AsyncHalI2cErrorKind,
    ErrorType as AsyncHalI2cErrorType, I2c, Operation, SevenBitAddress,
};

#[derive(Error, Debug)]
pub enum SoilSensorError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("I2c Read Error")]
    I2cReadError,
    #[error("I2c Write Error")]
    I2cWriteError,
}

impl AsyncHalI2cError for SoilSensorError {
    fn kind(&self) -> AsyncHalI2cErrorKind {
        // TODO
        AsyncHalI2cErrorKind::Other
    }
}

/// SoilSensor type designed to work with I2C mux device
/// TCA9548A
pub struct SoilSensor {
    i2c: AsyncWrapper,
    temp_delay: u64,
    moisture_delay: u64,
    address: u8,
}

/// Define a wrapper around the I2cSlave object returned by the
/// TCA9548A library, so we can implement the embedded_hal_async
/// I2c trait and build out an async framework.
/// TODO: Need to port this library as it no longer seems maitnained
pub struct AsyncWrapper {
    slave: I2cSlave<'static, Xca9548a<I2cdev>, I2cdev>,
}

unsafe impl Send for SoilSensor {}
unsafe impl Sync for SoilSensor {}

unsafe impl Send for AsyncWrapper {}
unsafe impl Sync for AsyncWrapper {}

impl AsyncHalI2cErrorType for AsyncWrapper {
    type Error = SoilSensorError;
}

impl I2c<SevenBitAddress> for AsyncWrapper {
    async fn transaction(
        &mut self,
        address: SevenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        operations
            .iter_mut()
            .map(|op| {
                match op {
                    Operation::Read(buffer) => {
                        self.slave
                            .read(address, buffer)
                            .map_err(|_| SoilSensorError::I2cReadError)?;
                    }
                    Operation::Write(buffer) => {
                        self.slave
                            .write(address, buffer)
                            .map_err(|_| SoilSensorError::I2cWriteError)?;
                    }
                }
                Ok::<(), Self::Error>(())
            })
            .take_while(Result::is_ok)
            .for_each(drop);
        Ok(())
    }
}

impl SoilSensor {
    pub fn new(i2c: I2cSlave<'static, Xca9548a<I2cdev>, I2cdev>, address: u8) -> Self {
        Self {
            i2c: AsyncWrapper { slave: i2c },
            temp_delay: 125,
            moisture_delay: 5000,
            address,
        }
    }

    pub async fn temperature(&mut self) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read(&mut buffer, &[0x00, 0x04], self.temp_delay)
            .await?;
        let raw = i32::from_be_bytes(buffer) as f32;
        // from https://github.com/adafruit/Adafruit_Seesaw/blob/master/Adafruit_seesaw.cpp#L810
        // and also convert celsius to fahrenheit
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }

    pub async fn moisture(&mut self) -> Result<u16, SoilSensorError> {
        let mut buffer = [0; 2];
        self.read(&mut buffer, &[0x0f, 0x10], self.moisture_delay)
            .await?;
        Ok(u16::from_be_bytes(buffer))
    }

    pub async fn read(
        &mut self,
        r_buffer: &mut [u8],
        w_buffer: &[u8],
        delay: u64,
    ) -> Result<(), SoilSensorError> {
        self.i2c
            .write(self.address, w_buffer)
            .await
            .map_err(|_| SoilSensorError::I2cWriteError)?;

        // from https://github.com/adafruit/Adafruit_Seesaw/blob/master/Adafruit_seesaw.cpp#L952
        sleep(Duration::from_micros(delay)).await;

        self.i2c
            .read(self.address, r_buffer)
            .await
            .map_err(|_| SoilSensorError::I2cReadError)?;

        Ok(())
    }
}
