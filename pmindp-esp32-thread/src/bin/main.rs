#![no_std]
#![no_main]

use esp_backtrace as _;

use esp_hal::{
    clock::ClockControl,
    gpio::Io,
    peripherals::Peripherals,
    prelude::*,
    system::SystemControl,
    timer::{systimer::SystemTimer, timg::TimerGroup},
};

#[cfg(feature = "atsamd10")]
use esp_hal::i2c::I2C;

#[cfg(feature = "probe-circuit")]
use esp_hal::{
    delay::Delay,
    gpio::{Level, Output},
};

use esp_println::println;
use pmindp_esp32_thread::init_heap;

use esp_ieee802154::Ieee802154;

#[entry]
fn main() -> ! {
    esp_println::logger::init_logger(log::LevelFilter::Info);

    init_heap();

    let mut peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();
    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    let mut ieee802154 = Ieee802154::new(peripherals.IEEE802154, &mut peripherals.RADIO_CLK);
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    cfg_if::cfg_if! {
        if #[cfg(feature="atsamd10")] {

            let sensor = pmindp_esp32_thread::ATSAMD10 {
                i2c: I2C::new(
                    peripherals.I2C0,
                    io.pins.gpio5,
                    io.pins.gpio6,
                    400.kHz(),
                    &clocks,
                ),
                temp_delay: 2000,
                moisture_delay: 5000,
                address: 0x36,
            };

            let mut platform = pmindp_esp32_thread::init(
                &mut ieee802154,
                &clocks,
                systimer.alarm0,
                TimerGroup::new(peripherals.TIMG0, &clocks),
                peripherals.RMT,
                io.pins.gpio8,
                peripherals.RNG,
                sensor
            );
        } else if #[cfg(feature="probe-circuit")] {

            let sensor = pmindp_esp32_thread::ProbeCircuit::new(
                Output::new(
                    io.pins.gpio6,
                    Level::Low
                ),
                io.pins.gpio2,
                peripherals.ADC1,
                Delay::new(&clocks)
            );

            let mut platform = pmindp_esp32_thread::init(
                &mut ieee802154,
                &clocks,
                systimer.alarm0,
                TimerGroup::new(peripherals.TIMG0, &clocks),
                peripherals.RMT,
                io.pins.gpio8,
                peripherals.RNG,
                sensor
            );

        } else {
            log::error!("No sensor target specified!");
            loop {

            }
        }
    }

    loop {
        // this will enter a loop where if it ever breaks,
        // we need to do a full reset
        // TODO find way to recover without resetting
        if platform.coap_server_event_loop().is_err() {
            println!("Unrecoverable error, resetting cpu!");
            platform.reset();
        }
    }
}
