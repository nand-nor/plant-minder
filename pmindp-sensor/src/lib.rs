//pub mod soil_sensor;

#[cfg(feature="local_async_i2cmux")]
use linux_embedded_hal::{I2cdev, i2cdev::linux::LinuxI2CError};
#[cfg(feature="std")]
use thiserror::Error;
#[cfg(feature="local_async_i2cmux")]
use xca9548a::{Error as I2cMuxError, Xca9548a, I2cSlave};


#[cfg(feature="std")]
#[derive(Error, Debug)]
pub enum SoilSensorError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Read Error")]
    I2cReadError,
    #[error("Write Error")]
    I2cWriteError,
    #[cfg(feature="local_async_i2cmux")]
    #[error("Mux Error")]
    I2cMuxError,
    #[error("Linux i2c error")]
    LinuxError(#[from]LinuxI2CError)
}

#[cfg(feature="local_async_i2cmux")]
impl<E> From<I2cMuxError<E>> for SoilSensorError {
    fn from(_error: I2cMuxError<E>) -> SoilSensorError {
        // todo logging
        SoilSensorError::I2cMuxError
    }
}

#[cfg(any(not(feature="std"), not(feature="local_async_i2cmux")))]
pub enum SoilSensorError {
    I2cReadError,
    I2cWriteError,
}


/// Seesaw I2C Soil Sensor 
pub struct ATSAMD10<I2C: PlantMinderI2c> {
    temp_delay: u32,
    moisture_delay: u32,
    address: u8,
    i2c: I2C
}

pub trait BaseSoilSensor {}

pub trait PlantMinderI2c {
    type Error;
    fn read(&self, address: u8,  r_buff:  &mut [u8]) -> Result<(), Self::Error>;
    fn write(&self, address: u8, w_buff: &[u8]) -> Result<(), Self::Error>;
}

#[cfg(feature="local_async_i2cmux")]
impl <'a>PlantMinderI2c for I2cSlave<'a, Xca9548a<I2cdev>,I2cdev> {
    type Error = SoilSensorError;

    fn read(&self, address: u8,  r_buff:  &mut [u8]) -> Result<(), Self::Error> {
        self.read(address, r_buff)
    }

    fn write(&self, address: u8, w_buff: &[u8]) -> Result<(), Self::Error> {
        self.write(address, w_buff)
    }
}


impl <I2C: PlantMinderI2c>BaseSoilSensor for ATSAMD10<I2C> {

}

pub struct Sensor<B: BaseSoilSensor> {
    sensor: B,
}

#[cfg(feature="local_async_i2cmux")]
unsafe impl <I2C: PlantMinderI2c>Send for ATSAMD10<I2C> {}
#[cfg(feature="local_async_i2cmux")]
unsafe impl <I2C:PlantMinderI2c>Sync for ATSAMD10<I2C> {}


impl <I2C: PlantMinderI2c>Sensor<ATSAMD10<I2C>> {
    const ATSAMD10_READ_MOISTURE: [u8;2] = [0x0f, 0x10];
    const ATSAMD10_READ_TEMP: [u8;2] = [0x00, 0x04];

    pub fn new(i2c: I2C, address: u8, temp_delay: u32, moisture_delay: u32) -> Self where I2C: PlantMinderI2c {
        Self {
            sensor: ATSAMD10 {
                i2c,
                temp_delay,
                moisture_delay,
                address,
            }
        }
    }

    #[cfg(feature="local_async_i2cmux")]
    pub async fn read_sensor(&self, r_buffer: &mut [u8],
        w_buffer: &[u8],
        delay: u32,
    ) -> Result<(), SoilSensorError> {
        
        self.sensor.i2c
            .write(self.sensor.address, w_buffer)
            .map_err(|_| SoilSensorError::I2cWriteError)?;
            
        // from https://github.com/adafruit/Adafruit_Seesaw/blob/master/Adafruit_seesaw.cpp#L952
        tokio::time::sleep(tokio::time::Duration::from_micros(delay.into())).await;
        
        self.sensor.i2c
            .read(self.sensor.address, r_buffer)
            .map_err(|_| SoilSensorError::I2cReadError)?;
         Ok(())
       
    }

    #[cfg(not(feature="local_async_i2cmux"))]
    pub fn read_sensor(&self, 
        r_buffer: &mut [u8],
        w_buffer: &[u8],  
        delay: u32,
        f: impl FnOnce(u32) -> (),
    ) -> Result<(), SoilSensorError> {
        
        self.sensor.i2c
            .write(self.sensor.address, w_buffer)
            .map_err(|_| SoilSensorError::I2cWriteError)?;
        f(delay);
        self.sensor.i2c
            .read(self.sensor.address, r_buffer)
            .map_err(|_| SoilSensorError::I2cReadError)?;
         Ok(())
       
    }

    #[cfg(not(feature="local_async_i2cmux"))]
    pub fn moisture(&self, f: impl FnOnce(u32) -> ()) -> Result<u16, SoilSensorError> {
        let mut buffer = [0; 2];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_MOISTURE, self.sensor.moisture_delay, f)?;
        Ok(u16::from_be_bytes(buffer))
    }

    #[cfg(not(feature="local_async_i2cmux"))]
    pub fn temperature(&self, f: impl FnOnce(u32) -> ()) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_TEMP, self.sensor.temp_delay, f)?;
        let raw = i32::from_be_bytes(buffer) as f32;
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }
    
    #[cfg(feature="local_async_i2cmux")]
    pub async fn moisture(&self) -> Result<u16, SoilSensorError>  {
        let mut buffer = [0; 2];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_MOISTURE, self.sensor.moisture_delay).await?;
        Ok(u16::from_be_bytes(buffer))
    }

    #[cfg(feature="local_async_i2cmux")]
    pub async fn temperature(&self) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_TEMP, self.sensor.temp_delay).await?;
        let raw = i32::from_be_bytes(buffer) as f32;
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }
}