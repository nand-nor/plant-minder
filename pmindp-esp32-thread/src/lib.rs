//! Build for remote sensor setup using espressif 802.15.4
//! capable dev boards (Currently only esp32-c6 and esp32-h2)
//! Use the espflash toolchain to build / flash / monitor

#![no_std]

// Compile time checks to prevent building with multiple
// sensor types
#[cfg(any(
    all(
        feature = "atsamd10",
        any(feature = "st0160", feature = "probe-circuit")
    ),
    all(
        feature = "st0160",
        any(feature = "atsamd10", feature = "probe-circuit")
    ),
    all(
        feature = "probe-circuit",
        any(feature = "atsamd10", feature = "st0160")
    )
))]
compile_error!("Cannot set multiple soil sensor types!");

// Compile time checks to prevent building with multiple gas sensors
#[cfg(all(feature = "bme680", feature = "sht40"))]
compile_error!("Cannot set multiple gas sensor types");

#[cfg(all(feature = "esp32h2", feature = "probe-circuit"))]
compile_error!("esp32h2 does not support the features neded to run the probe-circuit sensor");

extern crate alloc;

pub mod platform;
mod sensor;

pub use crate::{
    platform::Esp32Platform,
    sensor::{ATSAMD10, BME680, SHT40, TSL2591},
};

#[cfg(not(feature = "esp32h2"))]
pub use crate::sensor::ProbeCircuit;

use core::cell::RefCell;
use critical_section::Mutex;
use esp_hal::{
    interrupt::{self, Priority},
    peripherals::RNG,
    peripherals::{Interrupt, TIMG0},
    prelude::*,
    rng::Rng,
    timer::systimer::{Alarm, SpecificComparator, SpecificUnit, Target},
    timer::timg::{Timer, Timer0, TimerGroup},
    Blocking,
};
use esp_ieee802154::Ieee802154;

use pmindp_sensor::{Sensor, SensorPlatform};

use alloc::{boxed::Box, vec::Vec};

pub type SensorVec = Vec<Option<Mutex<RefCell<Box<dyn Sensor>>>>>;

type SensorTimer = Mutex<RefCell<Option<Timer<Timer0<TIMG0>, Blocking>>>>;

static SENSOR_TIMER: SensorTimer = Mutex::new(RefCell::new(None));
const DEFAULT_MIN_INTERVAL: u64 = 5000;
static SENSOR_TIMER_INTERVAL: Mutex<RefCell<u64>> = Mutex::new(RefCell::new(DEFAULT_MIN_INTERVAL));
static SENSOR_TIMER_FIRED: Mutex<RefCell<bool>> = Mutex::new(RefCell::new(false));

pub fn init<'a>(
    ieee802154: &'a mut Ieee802154,
    timer: Alarm<
        'static,
        Target,
        Blocking,
        SpecificComparator<'static, 0>,
        SpecificUnit<'static, 0>,
    >,
    timg0: TimerGroup<TIMG0, Blocking>,
    rng: RNG,
    sensors: SensorVec,
) -> Esp32Platform<'a>
where
    Esp32Platform<'a>: SensorPlatform,
{
    let openthread = esp_openthread::OpenThread::new(ieee802154, timer, Rng::new(rng));
    let timer = timg0.timer0;
    setup_sensor_timer(timer, 25000);

    Esp32Platform::new(openthread, sensors)
}

#[handler]
pub fn SENSOR_TIMER_TG0_T0_LEVEL() {
    log::trace!("sensor timer interrupt triggered");
    critical_section::with(|cs| {
        *SENSOR_TIMER_FIRED.borrow_ref_mut(cs) = true;
        let mut timer = SENSOR_TIMER.borrow_ref_mut(cs);
        let timer = timer.as_mut().unwrap();
        let interval = SENSOR_TIMER_INTERVAL.borrow_ref(cs);
        timer.clear_interrupt();
        timer.load_value(interval.millis()).unwrap();
        timer.start();
    });
}

fn setup_sensor_timer(timer: Timer<Timer0<TIMG0>, Blocking>, interval: u64) {
    timer.set_interrupt_handler(SENSOR_TIMER_TG0_T0_LEVEL);

    timer.clear_interrupt();

    interrupt::enable(Interrupt::TG0_T0_LEVEL, Priority::Priority1).unwrap();
    timer.load_value(interval.millis()).unwrap();
    timer.start();
    timer.listen();

    critical_section::with(|cs| {
        SENSOR_TIMER.borrow_ref_mut(cs).replace(timer);
        *SENSOR_TIMER_INTERVAL.borrow_ref_mut(cs) = interval;
    });
}
