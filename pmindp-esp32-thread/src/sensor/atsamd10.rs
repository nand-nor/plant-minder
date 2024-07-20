/// [Seesaw soil sensor](https://www.adafruit.com/product/4026)
/// This is the default soil sensor for programming 
/// dev boards


use embedded_hal::i2c::I2c;

use pmindp_sensor::{SoilSensor, SoilSensorError};

use core::ops::FnOnce;
use core::result::Result;
use core::result::Result::Ok;

/// Seesaw I2C Soil Sensor
pub struct ATSAMD10<I2C: I2c> {
    pub temp_delay: u32,
    pub moisture_delay: u32,
    pub address: u8,
    pub i2c: I2C,
}

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

impl<I2C> SoilSensor for ATSAMD10<I2C>
where
    I2C: I2c,
{
    type InputParams = alloc::boxed::Box<dyn FnOnce(u32)>;

    type MoistureOutput = u16;

    type TemperatureOutput = f32;

    fn moisture(
        &mut self,
        r: Self::InputParams,
    ) -> Result<Self::MoistureOutput, crate::SoilSensorError> {
        self.moisture(r)
    }

    fn temperature(
        &mut self,
        r: Self::InputParams,
    ) -> Result<Self::TemperatureOutput, crate::SoilSensorError> {
        self.temperature(r)
    }
}
