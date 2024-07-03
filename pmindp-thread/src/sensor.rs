
use esp_hal::{
    clock::ClockControl,
    delay::Delay,
    gpio::Io,
    i2c::I2C,
    prelude::*,
    interrupt::{self, Priority},
    peripherals::{I2C0, Interrupt, Peripherals, TIMG0},
    system::SystemControl,
    timer::timg::{Timer, Timer0, TimerGroup, TimerInterrupts}, Blocking,
};

use core::{
    borrow::BorrowMut,
    cell::RefCell,
    marker::{PhantomData, PhantomPinned},
};

use critical_section::Mutex;

static I2C_SENSOR: Mutex<RefCell<Option<&'static mut I2C<I2C0, Blocking>>>> = Mutex::new(RefCell::new(None));

static SENSOR_TIMER:  Mutex<RefCell<Option<Timer<Timer0<TIMG0>, esp_hal::Blocking>>>> =
    Mutex::new(RefCell::new(None));

const DEFAULT_MIN_INTERVAL:u64 = 5000;

static SENSOR_TIMER_INTERVAL: Mutex<RefCell<u64>> =
    Mutex::new(RefCell::new(DEFAULT_MIN_INTERVAL));    

static SENSOR_TIMER_FIRED: Mutex<RefCell<bool>> =
    Mutex::new(RefCell::new(false));    

fn with_sensor<F, T>(f: F) -> Option<T>
where
    F: FnOnce(& mut I2C<I2C0, Blocking>) -> T,
{
    critical_section::with(|cs| {
        let mut sensor = I2C_SENSOR.borrow_ref_mut(cs);
        let sensor = sensor.borrow_mut();

        if let Some(sensor) = sensor.as_mut() {
            Some(f(sensor))
        } else {
            None
        }
    })
}

pub enum SoilSensorError {
    I2cReadError,
    I2cWriteError,
    OtherError,
}

pub struct I2cSoilSensor<'a> {
    _phantom: PhantomData<&'a ()>,
    temp_delay: u32,
    moisture_delay: u32,
    address: u8,
    delay: Delay,
}

impl <'a>I2cSoilSensor<'a> {
    pub fn new(
        i2c: &'a mut I2C<I2C0, Blocking>,
        interval: u64,
        mut timg0: TimerGroup<TIMG0, Blocking>,
        delay: Delay,
    ) -> Self {
    
        let timer = timg0.timer0;
    
        setup_sensor_timer(timer, interval);

        critical_section::with(|cs| unsafe {
            I2C_SENSOR
                .borrow_ref_mut(cs)
                .replace(core::mem::transmute(i2c));
        });


      Self {
        _phantom: PhantomData,
        temp_delay: 2000,
        moisture_delay: 5000,
        address: 0x36, 
        delay,
      }
    }

    pub fn read(&self, r_buffer: &mut [u8],
        w_buffer: &[u8],
        delay: u32,
    ) -> Result<(), SoilSensorError> {
        with_sensor(|i2c| {
            i2c
            .write(self.address, w_buffer)
                .map_err(|_| SoilSensorError::I2cWriteError)?;
            // from https://github.com/adafruit/Adafruit_Seesaw/blob/master/Adafruit_seesaw.cpp#L952
            self.delay.delay_micros(delay);
            i2c
                .read(self.address, r_buffer)
                .map_err(|_| SoilSensorError::I2cReadError)?;
            Ok::<(), SoilSensorError>(())
        }).ok_or(|e: SoilSensorError| e);
        Ok(())
    }

    pub fn moisture(&self) -> Result<u16, SoilSensorError> {
        let mut buffer = [0; 2];
        self.read(&mut buffer, &[0x0f, 0x10], self.moisture_delay)?;
        log::debug!("Pulled moisture {:?}", buffer);
        Ok(u16::from_be_bytes(buffer))
    }

    pub fn temperature(&self) -> Result<f32, SoilSensorError> {
        let mut buffer = [0; 4];
        self.read(&mut buffer, &[0x00, 0x04], self.temp_delay)?;
        log::debug!("Pulled temp {:?}", buffer);
        let raw = i32::from_be_bytes(buffer) as f32;
        // from https://github.com/adafruit/Adafruit_Seesaw/blob/master/Adafruit_seesaw.cpp#L810
        // convert celsius to fahrenheit
        let raw = (1.0 / 1_i32.wrapping_shl(16) as f32) * raw;
        Ok((raw * 1.8) + 32.0)
    }
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

fn setup_sensor_timer(mut timer: Timer<Timer0<TIMG0>, esp_hal::Blocking>, interval: u64) {
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

pub fn sensor_read<'a>(i2c_wrapper: &I2cSoilSensor<'a>) -> Result<Option<(u16, f32)>, SoilSensorError> {
    let read_sensor = critical_section::with(|cs| {
        let res = *SENSOR_TIMER_FIRED.borrow_ref_mut(cs);
        *SENSOR_TIMER_FIRED.borrow_ref_mut(cs) = false;
        res
    });

    if read_sensor {
       let moisture = i2c_wrapper.moisture()?;
       let temp = i2c_wrapper.temperature()?;
       Ok(Some((moisture, temp)))
    } else {
        Ok(None)
    }
    
}