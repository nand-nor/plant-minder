//! Spark fun soil moisture sensor
//! uses a simple probe circuit
//! https://cdn.sparkfun.com/datasheets/Sensors/Biometric/SparkFun_Soil_Moisture_Sensor.p
use esp_hal::{
    analog::adc::{Adc, AdcConfig, AdcPin, Attenuation},
    delay::Delay,
    gpio::{GpioPin, Output},
    peripherals::ADC1,
    prelude::*,
};

use pmindp_sensor::{MoistureSensor, PlatformSensorError, Sensor, SoilSensorError};

type AdcCal = esp_hal::analog::adc::AdcCalCurve<ADC1>;
//type AdcCal = esp_hal::analog::adc::AdcCalLine<esp_hal::peripherals::ADC1>;
//type AdcCal = esp_hal::analog::adc::AdcCalBasic<esp_hal::peripherals::ADC1>;

//impl SensorReading for ProbeCircuitSensorReading {}

pub struct ProbeCircuit<'a> {
    // digital pin
    pwr_pin: Output<'a, GpioPin<6>>,
    // analog pin
    sensor_pin: AdcPin<GpioPin<2>, ADC1, AdcCal>,
    adc1: Adc<'a, ADC1>,
    delay: Delay,
}

impl<'a> ProbeCircuit<'a> {
    pub fn new(
        pwr_pin: Output<'a, GpioPin<6>>,
        sensor_pin: GpioPin<2>,
        adc1: ADC1,
        delay: Delay,
    ) -> Self {
        let mut adc1_config = AdcConfig::new();

        // TODO consider putting attenuation and calibration type behind feature flag?
        let adc1_pin =
            adc1_config.enable_pin_with_cal::<_, AdcCal>(sensor_pin, Attenuation::Attenuation11dB);

        let adc1 = Adc::new(adc1, adc1_config);

        Self {
            sensor_pin: adc1_pin,
            pwr_pin,
            delay,
            adc1,
        }
    }

    fn moisture(&mut self) -> Result<u16, SoilSensorError> {
        self.pwr_pin.set_high();
        // delay 10 millis
        self.delay.delay_micros(10000);
        let val = nb::block!(self.adc1.read_oneshot(&mut self.sensor_pin)).unwrap();
        self.pwr_pin.set_low();
        Ok(val)
    }
}

impl<'a> MoistureSensor for ProbeCircuit<'a> {
    fn moisture(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, SoilSensorError> {
        let reading = self.moisture().map_err(SoilSensorError::from)?;
        log::info!("moisture {:?}", reading);

        let size = core::mem::size_of::<u16>();
        buffer[start..start + size].copy_from_slice(&reading.to_le_bytes());
        Ok(size)
    }
}

impl<'a> Sensor for ProbeCircuit<'a> {
    fn read(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, PlatformSensorError> {
        let size = <Self as MoistureSensor>::moisture(self, buffer, start)
            .map_err(PlatformSensorError::from)?;
        Ok(size)
    }
}
