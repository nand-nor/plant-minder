//! Sht4x temperature / humidity / pressure / gas
//! sensor from adafruit

use embedded_hal::i2c::I2c;
use esp_hal::delay::Delay;
use sht4x::*;

use pmindp_sensor::{Gas, PlatformSensorError, Sensor};

pub struct SHT40<I2C: I2c> {
    delay: Delay,
    sensor: Sht4x<I2C, Delay>,
    precision: sht4x::Precision,
}

impl<I2C> SHT40<I2C>
where
    I2C: I2c,
{
    pub fn new(i2c: I2C, delay: Delay) -> Result<Self, PlatformSensorError> {
        let sensor = Sht4x::new(i2c);
        Ok(Self {
            delay,
            sensor,
            precision: sht4x::Precision::Medium,
        })
    }

    fn read_sensor(&mut self) -> Result<Gas, PlatformSensorError> {
        let mut delay = self.delay;
        let data = self
            .sensor
            .measure(self.precision, &mut delay)
            .map_err(|e| {
                log::error!("Error getting sensor data {e:?}");
                PlatformSensorError::Other
            })?;

        // convert to Fs
        let temp = (data.temperature_celsius().to_num::<f32>() * 1.8) + 32.0;
        let h = data.humidity_percent().to_num::<f32>();
        let gas = 0; // sensor does not have this capability
        let p = 0.0; // sensor does not have this capability

        Ok(Gas { temp, p, h, gas })
    }
}

impl<I2C> Sensor for SHT40<I2C>
where
    I2C: I2c,
{
    fn read(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, PlatformSensorError> {
        let reading: Gas = self.read_sensor()?;
        let reading = serde_json::to_vec(&reading).map_err(|e| {
            log::error!("Serde failed {e:}");
            PlatformSensorError::Other
        })?;

        let len = reading.len();
        buffer[start..start + len].copy_from_slice(&reading);

        Ok(len)
    }
}
