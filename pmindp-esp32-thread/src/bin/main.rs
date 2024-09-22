#![no_std]
#![no_main]

use esp_backtrace as _;

use esp_hal::{
    delay::Delay,
    gpio::Io,
    i2c::I2C,
    prelude::*,
    timer::{
        systimer::{Alarm, FrozenUnit, SpecificUnit, SystemTimer},
        timg::TimerGroup,
    },
};

extern crate alloc;

use alloc::boxed::Box;
use static_cell::StaticCell;

#[cfg(feature = "probe-circuit")]
use esp_hal::gpio::{Level, Output};

use esp_println::println;
use pmindp_esp32_thread::SensorVec;

use esp_ieee802154::Ieee802154;

use core::cell::RefCell;
use embedded_hal_bus::i2c;

use critical_section::Mutex;

#[entry]
fn main() -> ! {
    esp_println::logger::init_logger(log::LevelFilter::Info);

    esp_alloc::heap_allocator!(64 * 1024);

    let mut peripherals = esp_hal::init(esp_hal::Config::default());

    let mut ieee802154 = Ieee802154::new(peripherals.IEEE802154, &mut peripherals.RADIO_CLK);
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    static UNIT0: StaticCell<SpecificUnit<'static, 0>> = StaticCell::new();
    let unit0 = UNIT0.init(systimer.unit0);
    let frozen_unit = FrozenUnit::new(unit0);
    let alarm = Alarm::new(systimer.comparator0, &frozen_unit);

    #[cfg(not(feature = "esp32h2"))]
    let i2c = I2C::new(peripherals.I2C0, io.pins.gpio5, io.pins.gpio6, 400.kHz());
    #[cfg(feature = "esp32h2")]
    let i2c = I2C::new(peripherals.I2C0, io.pins.gpio2, io.pins.gpio3, 400.kHz());

    let i2c_ref_cell = RefCell::new(i2c);
    let i2c_ref_cell: &'static _ = Box::leak(Box::new(i2c_ref_cell));

    let mut sensors: SensorVec = (0..pmindp_sensor::MAX_SENSORS).map(|_| None).collect();

    // Require at least a moisture sensor
    cfg_if::cfg_if! {
        if #[cfg(feature="atsamd10")] {
            let soil_sensor = pmindp_esp32_thread::ATSAMD10 {
                i2c:  i2c::RefCellDevice::new(i2c_ref_cell),
                temp_delay: 2000,
                moisture_delay: 5000,
                address: 0x36,
                delay: Delay::new()
            };

            sensors.insert(pmindp_sensor::SOIL_IDX, Some(Mutex::new(RefCell::new(Box::new(soil_sensor)))));

        } else if #[cfg(feature="probe-circuit")] {
            let soil_sensor = pmindp_esp32_thread::ProbeCircuit::new(
                Output::new(
                    io.pins.gpio4,
                    Level::Low
                ),
                io.pins.gpio2,
                peripherals.ADC1,
                Delay::new()
            );
            sensors.insert(pmindp_sensor::SOIL_IDX, Some(Mutex::new(RefCell::new(Box::new(soil_sensor)))));
        } else {
            log::error!("No sensor target specified!");
            panic!("No sensors specified")
        }
    }

    // enable optional light sensor configuration (if one is specified)
    cfg_if::cfg_if! {
        if #[cfg(feature="tsl2591")] {
            let light_sensor = pmindp_esp32_thread::TSL2591::new(
                i2c::RefCellDevice::new(i2c_ref_cell),
                0x29,
                Delay::new()
            ).unwrap();
            sensors.insert(pmindp_sensor::LIGHT_IDX_1, Some(Mutex::new(RefCell::new(Box::new(light_sensor)))));
        }
    }

    // enable humidity/gas sensor configuration (if one is specified)
    cfg_if::cfg_if! {
        if #[cfg(feature="bme680")] {
            let gas_sensor = pmindp_esp32_thread::BME680::new(
                i2c::RefCellDevice::new(i2c_ref_cell),
                Delay::new()
            ).unwrap();
            sensors.insert(pmindp_sensor::HUM_IDX, Some(Mutex::new(RefCell::new(Box::new(gas_sensor)))));
        } else if #[cfg(feature="sht40")] {
            let gas_sensor = pmindp_esp32_thread::SHT40::new(
                i2c::RefCellDevice::new(i2c_ref_cell),
                Delay::new()
            ).unwrap();
            sensors.insert(pmindp_sensor::HUM_IDX, Some(Mutex::new(RefCell::new(Box::new(gas_sensor)))));
        }
    }

    let mut platform = pmindp_esp32_thread::init(
        &mut ieee802154,
        alarm,
        TimerGroup::new(peripherals.TIMG0),
        peripherals.RNG,
        sensors,
    );

    loop {
        // this will enter a loop where if it ever breaks,
        // we need to do a full reset
        // TODO find way to recover without resetting
        if platform.main_event_loop().is_err() {
            println!("Unrecoverable error, resetting cpu!");
            platform.reset();
        }
    }
}
