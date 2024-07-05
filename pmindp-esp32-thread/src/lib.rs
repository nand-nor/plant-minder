//! Build for remote sensor setup using espressif 802154
//! capable dev boards (Currently only esp32-c6 and esp32-h2)
//! Use the espflash toolchain to build / flash / monitor

#![no_std]

mod sensor;

use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};

use esp_hal::{
    clock::Clocks,
    gpio::GpioPin,
    peripheral::Peripheral,
    peripherals::RMT,
    prelude::*,
    rmt::{Channel, Rmt},
    Blocking,
};

pub use sensor::{sensor_setup, sensor_read, SENSOR_TIMER_TG0_T0_LEVEL};

pub fn led_setup(
    rmt: impl Peripheral<P = RMT>,
    led_pin: GpioPin<8>,
    clocks: &Clocks,
) -> SmartLedsAdapter<Channel<Blocking, 0>, 25> {
    #[cfg(not(feature = "esp32h2"))]
    let rmt = Rmt::new(rmt, 80.MHz(), clocks, None).unwrap();
    #[cfg(feature = "esp32h2")]
    let rmt = Rmt::new(rmt, 32.MHz(), &clocks, None).unwrap();

    let rmt_buffer = smartLedBuffer!(1);
    SmartLedsAdapter::new(rmt.channel0, led_pin, rmt_buffer, &clocks)
}
