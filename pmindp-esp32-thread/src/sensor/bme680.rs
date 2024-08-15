/// bme680 temperature / humidity / pressure / gas
/// sensor from adafruit

use bme680::*;
use embedded_hal::i2c::I2c;
use esp_hal::delay::Delay;

use pmindp_sensor::{Gas, PlatformSensorError, Sensor};

pub struct BME680<I2C: I2c> {
    delay: Delay,
    sensor: Bme680<I2C, Delay>,
}

impl<I2C> BME680<I2C>
where
    I2C: I2c,
{
    pub fn new(i2c: I2C, mut delay: Delay) -> Result<Self, PlatformSensorError> {
        let mut sensor = Bme680::init(i2c, &mut delay, I2CAddress::Secondary).map_err(|e| {
            log::error!("Unable to init BME680, setup error {e:?}");
            PlatformSensorError::SensorSetup
        })?;

        let settings = SettingsBuilder::new()
            .with_humidity_oversampling(OversamplingSetting::OS2x)
            .with_pressure_oversampling(OversamplingSetting::OS4x)
            //.with_temperature_oversampling(OversamplingSetting::OS8x)
            //.with_temperature_filter(IIRFilterSize::Size3)
            .with_gas_measurement(core::time::Duration::from_millis(1500), 320, 25)
            .with_temperature_offset(-2.2)
            .with_run_gas(true)
            .build();

        sensor
            .set_sensor_settings(&mut delay, settings)
            .map_err(|e| {
                log::error!("Setup error {e:?}");
                PlatformSensorError::SensorSetup
            })?;
        sensor
            .set_sensor_mode(&mut delay, PowerMode::ForcedMode)
            .map_err(|e| {
                log::error!("Setup error {e:?}");
                PlatformSensorError::SensorSetup
            })?;

        Ok(Self { delay, sensor })
    }

    fn read_sensor(&mut self) -> Result<Gas, PlatformSensorError> {
        let mut delay = self.delay;
        let (data, _state) = self.sensor.get_sensor_data(&mut delay).map_err(|e| {
            log::error!("Error getting sensor data {e:?}");
            PlatformSensorError::Other
        })?;

        // convert to Fs
        let temp = (data.temperature_celsius() * 1.8) + 32.0;
        let h = data.humidity_percent();
        let gas = data.gas_resistance_ohm();
        let p = data.pressure_hpa();

        self.sensor
            .set_sensor_mode(&mut delay, PowerMode::ForcedMode)
            .map_err(|e| {
                log::error!("Error setting sensor powermode {e:?}");
                PlatformSensorError::Other
            })?;

        Ok(Gas {
            temp,
            p,
            h,
            gas,
        })
    }
}

impl<I2C> Sensor for BME680<I2C>
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
