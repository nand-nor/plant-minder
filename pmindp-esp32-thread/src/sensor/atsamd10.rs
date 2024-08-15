/// [Seesaw soil sensor](https://www.adafruit.com/product/4026)
use embedded_hal::i2c::I2c;
use esp_hal::delay::Delay;
use pmindp_sensor::{MoistureSensor, PlatformSensorError, Sensor, SoilSensorError, TempSensor};

/// Seesaw I2C Soil Sensor
pub struct ATSAMD10<I2C: I2c> {
    pub temp_delay: u32,
    pub moisture_delay: u32,
    pub address: u8,
    pub i2c: I2C,
    pub delay: Delay,
}

impl<I2C: I2c> ATSAMD10<I2C> {
    const ATSAMD10_READ_MOISTURE: [u8; 2] = [0x0f, 0x10];
    const ATSAMD10_READ_TEMP: [u8; 2] = [0x00, 0x04];

    pub fn new(i2c: I2C, address: u8, temp_delay: u32, moisture_delay: u32, delay: Delay) -> Self {
        Self {
            i2c,
            temp_delay,
            moisture_delay,
            address,
            delay,
        }
    }

    pub fn read_sensor(
        &mut self,
        r_buffer: &mut [u8],
        w_buffer: &[u8],
        delay: u32,
    ) -> Result<(), SoilSensorError> {
        self.i2c
            .write(self.address, w_buffer)
            .map_err(|_| SoilSensorError::I2cWriteError)?;
        self.delay.delay_micros(delay);
        self.i2c
            .read(self.address, r_buffer)
            .map_err(|_| SoilSensorError::I2cReadError)?;
        Ok(())
    }

    pub fn moisture(&mut self) -> Result<u16, SoilSensorError> {
        let mut buffer = [0; 2];
        self.read_sensor(
            &mut buffer,
            &Self::ATSAMD10_READ_MOISTURE,
            self.moisture_delay,
        )?;
        Ok(u16::from_be_bytes(buffer))
    }

    pub fn temperature(&mut self) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read_sensor(&mut buffer, &Self::ATSAMD10_READ_TEMP, self.temp_delay)?;
        let raw = i32::from_be_bytes(buffer) as f32;
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }
}

#[allow(unused)]
impl<I2C> MoistureSensor for ATSAMD10<I2C>
where
    I2C: I2c,
{
    fn moisture(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, SoilSensorError> {
        let reading = self.moisture().map_err(SoilSensorError::from)?;
        let size = core::mem::size_of::<u16>();
        buffer[start..start + size].copy_from_slice(&reading.to_le_bytes());
        Ok(size)
    }
}

#[allow(unused)]
impl<I2C> TempSensor for ATSAMD10<I2C>
where
    I2C: I2c,
{
    fn temp(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, SoilSensorError> {
        let reading = self.temperature().map_err(SoilSensorError::from)?;
        let size = core::mem::size_of::<f32>();
        buffer[start..start + size].copy_from_slice(&reading.to_le_bytes());
        Ok(size)
    }
}

impl<I2C> Sensor for ATSAMD10<I2C>
where
    I2C: I2c,
{
    fn read(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, PlatformSensorError> {
        let temp = self.temperature().map_err(SoilSensorError::from)?;
        log::debug!("temp {:?}", temp);
        let moisture = self.moisture().map_err(SoilSensorError::from)?;
        log::debug!("moisture {:?}", moisture);

        let reading: pmindp_sensor::Soil = pmindp_sensor::Soil {
            moisture,
            temp,
        };

        let reading = serde_json::to_vec(&reading).map_err(|e| {
            log::error!("Serde failed {e:}");
            PlatformSensorError::Other
        })?;
        let len = reading.len();
        buffer[start..start + len].copy_from_slice(&reading);
        Ok(len)
    }
}
