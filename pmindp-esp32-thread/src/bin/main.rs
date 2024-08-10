#![no_std]
#![no_main]

use esp_backtrace as _;

use esp_hal::{
    clock::ClockControl,
    delay::Delay,
    gpio::Io,
    i2c::I2C,
    peripherals::Peripherals,
    prelude::*,
    system::SystemControl,
    timer::{systimer::SystemTimer, timg::TimerGroup},
};

extern crate alloc;

//#[cfg(feature = "atsamd10")]
//use esp_hal::i2c::I2C;
use alloc::{boxed::Box, vec::Vec};

use pmindp_sensor::Sensor;

#[cfg(feature = "probe-circuit")]
use esp_hal::gpio::{Level, Output};

use esp_println::println;
use pmindp_esp32_thread::init_heap;

use esp_ieee802154::Ieee802154;

use core::cell::RefCell;
use embedded_hal_bus::i2c;

use critical_section::Mutex;

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


    #[cfg(not(feature = "esp32h2"))]
    let i2c = I2C::new(
        peripherals.I2C0,
        io.pins.gpio5,
        io.pins.gpio6,
        400.kHz(),
        &clocks,
    );
    #[cfg(feature = "esp32h2")]
    let i2c = I2C::new(
        peripherals.I2C0,
        io.pins.gpio2,
        io.pins.gpio3,
        400.kHz(),
        &clocks,
    );    

    let i2c_ref_cell = RefCell::new(i2c);
    let i2c_ref_cell: &'static _ = Box::leak(Box::new(i2c_ref_cell));

    let mut sensors: Vec<Mutex<RefCell<Box<dyn Sensor>>>> =
        Vec::with_capacity(pmindp_sensor::MAX_SENSORS);

    // Require at least a moisture sensor
    cfg_if::cfg_if! {
        if #[cfg(feature="atsamd10")] {
            let soil_sensor = pmindp_esp32_thread::ATSAMD10 {
                i2c:  i2c::RefCellDevice::new(i2c_ref_cell),
                temp_delay: 2000,
                moisture_delay: 5000,
                address: 0x36,
                delay: Delay::new(&clocks)
            };

            sensors.insert(pmindp_sensor::SOIL_IDX, Mutex::new(RefCell::new(Box::new(soil_sensor))));

        } else if #[cfg(feature="probe-circuit")] {
            let soil_sensor = pmindp_esp32_thread::ProbeCircuit::new(
                Output::new(
                    io.pins.gpio4,
                    Level::Low
                ),
                io.pins.gpio2,
                peripherals.ADC1,
                Delay::new(&clocks)
            );
            sensors.insert(pmindp_sensor::SOIL_IDX, Mutex::new(RefCell::new(Box::new(soil_sensor))));
        } else {
            log::error!("No sensor target specified!");
            panic!("No sensors specified")
        }
    }

    // optionally enable light sensor as well
    cfg_if::cfg_if! {
        if #[cfg(feature="tsl2591")] {
            let light_sensor = pmindp_esp32_thread::TSL2591::new(
                i2c::RefCellDevice::new(i2c_ref_cell),
                0x29,
                Delay::new(&clocks)
            ).unwrap();
            sensors.insert(pmindp_sensor::LIGHT_IDX_1, Mutex::new(RefCell::new(Box::new(light_sensor))));
        }
    }

    let mut platform = pmindp_esp32_thread::init(
        &mut ieee802154,
        &clocks,
        systimer.alarm0,
        TimerGroup::new(peripherals.TIMG0, &clocks),
        peripherals.RMT,
        io.pins.gpio8,
        peripherals.RNG,
        sensors,
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
