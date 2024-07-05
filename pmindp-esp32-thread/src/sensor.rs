use esp_hal::{
    delay::Delay,
    i2c::I2C,
    interrupt::{self, Priority},
    peripherals::{Interrupt, I2C0, TIMG0},
    prelude::*,
    timer::timg::{Timer, Timer0, TimerGroup},
    Blocking,
};

use pmindp_sensor::{SoilSensorError, ATSAMD10};

use core::{
    borrow::BorrowMut,
    cell::RefCell,
};

use critical_section::Mutex;

static I2C_SENSOR: Mutex<RefCell<Option<ATSAMD10<&mut I2C<I2C0, Blocking>>>>> =
    Mutex::new(RefCell::new(None));

static SENSOR_TIMER: Mutex<RefCell<Option<Timer<Timer0<TIMG0>, esp_hal::Blocking>>>> =
    Mutex::new(RefCell::new(None));

const DEFAULT_MIN_INTERVAL: u64 = 5000;

static SENSOR_TIMER_INTERVAL: Mutex<RefCell<u64>> = Mutex::new(RefCell::new(DEFAULT_MIN_INTERVAL));

static SENSOR_TIMER_FIRED: Mutex<RefCell<bool>> = Mutex::new(RefCell::new(false));


pub fn sensor_setup<'a>(
    i2c: &'a mut I2C<'a, I2C0, Blocking>,
    interval: u64,
    timg0: TimerGroup<TIMG0, Blocking>,
) {
    let timer = timg0.timer0;
    setup_sensor_timer(timer, interval);

    // Read / Write / methods for pulling moisture and temp are defined in
    // pmindp-sensor
    let sensor = ATSAMD10 {
        i2c,
        temp_delay: 2000,
        moisture_delay: 5000,
        address: 0x36,
    };

    critical_section::with(|cs| unsafe {
        I2C_SENSOR
            .borrow_ref_mut(cs)
            .replace(core::mem::transmute(sensor));
    });
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

fn setup_sensor_timer(timer: Timer<Timer0<TIMG0>, esp_hal::Blocking>, interval: u64) {
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

pub fn sensor_read(delay: Delay) -> Result<Option<(u16, f32)>, SoilSensorError> {
    let read_sensor = critical_section::with(|cs| {
        let res = *SENSOR_TIMER_FIRED.borrow_ref_mut(cs);
        *SENSOR_TIMER_FIRED.borrow_ref_mut(cs) = false;
        res
    });

    if read_sensor {
        let res = critical_section::with(|cs| {
            let mut i2c = I2C_SENSOR.borrow_ref_mut(cs);
            let i2c = i2c.borrow_mut();
            if let Some(i2c) = i2c.as_mut() {
                let m_delay = i2c.moisture_delay;
                let t_delay = i2c.temp_delay;
                let moisture = i2c.moisture(|_| delay.delay_micros(m_delay))?;
                let temp = i2c.temperature(|_| delay.delay_micros(t_delay))?;
                Ok(Some((moisture, temp)))
            } else {
                Ok(None)
            }
        })
        .map_err(|e| {
            log::error!("Error reading from sensor");
            e
        });
        res
    } else {
        Ok(None)
    }
}