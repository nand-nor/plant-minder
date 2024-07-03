#![no_std]

pub mod sensor;

pub use sensor::{I2cSoilSensor, SENSOR_TIMER_TG0_T0_LEVEL, sensor_read};