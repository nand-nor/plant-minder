#![no_std]
#![no_main]

use esp_backtrace as _;

use esp_hal::{
    clock::ClockControl,
    gpio::Io,
    peripherals::Peripherals,
    prelude::*,
    system::SystemControl,
    timer::{
        systimer::SystemTimer,
        timg::{TimerGroup, TimerInterrupts},
    },
};
use esp_println::println;
use pmindp_esp32_thread::{init_heap, Esp32Platform, SENSOR_TIMER_TG0_T0_LEVEL};

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

    let mut platform = Esp32Platform::new(
        &mut ieee802154,
        &clocks,
        systimer.alarm0,
        peripherals.I2C0,
        TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            Some(TimerInterrupts {
                timer0: Some(SENSOR_TIMER_TG0_T0_LEVEL),
                ..Default::default()
            }),
        ),
        peripherals.RMT,
        io.pins.gpio8,
        io.pins.gpio5,
        io.pins.gpio6,
        peripherals.RNG,
    );

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
