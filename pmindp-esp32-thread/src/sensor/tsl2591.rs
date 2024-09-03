//! TSL2951 light sensor
//! port of https://github.com/adafruit/Adafruit_TSL2591_Library
use embedded_hal::i2c::I2c;

use core::ops::{BitAnd, BitOr, Shl, Shr};
use esp_hal::delay::Delay;
use pmindp_sensor::{
    I2cError, LightLumenSensor, LightLuxSensor, LightSensorError, PlatformSensorError, Sensor,
};

#[derive(Debug)]
pub enum Tsl2591Error {
    I2cError(I2cError),
    SetupError,
    SensorError,
    SignalOverflow,
}

impl From<Tsl2591Error> for LightSensorError {
    fn from(e: Tsl2591Error) -> Self {
        match e {
            Tsl2591Error::I2cError(i) => LightSensorError::I2cError(i),
            Tsl2591Error::SensorError => LightSensorError::SensorError,
            Tsl2591Error::SignalOverflow => LightSensorError::SignalOverflow,
            _ => LightSensorError::SetupError,
        }
    }
}

impl From<I2cError> for Tsl2591Error {
    fn from(e: I2cError) -> Self {
        Tsl2591Error::I2cError(e)
    }
}

#[derive(Default, Clone, Debug)]
pub enum Gain {
    Low = 0,
    #[default]
    Medium = 0x10,
    High = 0x20,
    Max = 0x30,
}

#[derive(Default, Clone)]
pub enum Mode {
    /// channel 0
    #[default]
    FullSpectrum = 0,
    /// channel 1
    Infrared = 1,
    /// (channel 0) - (channel 1)
    Visible = 2,
}

#[derive(Default, Clone)]
#[repr(u8)]
pub enum IntegrationTime {
    IntTime100 = 0x0,
    IntTime200 = 0x1,
    #[default]
    IntTime300 = 0x2,
    IntTime400 = 0x3,
    IntTime500 = 0x4,
    IntTime600 = 0x5,
}

// from https://github.com/adafruit/Adafruit_Library/blob/master/Adafruit_TSL2591.h
bitflags::bitflags! {
    /// TSL2591 register map
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(C)]
    struct RegisterMap: u8 {
        /// Engable register
        const ENABLE= 0x00;
        /// Control register
        const CONTROL= 0x01;

        /// ALS low threshold low byte
        const THRESHOLD_AILTL= 0x04;
        /// ALS low threshold higher byte
        const THRESHOLD_AILTH= 0x05;
        /// ALS high threshold lower byte
        const THRESHOLD_AIHTL= 0x06;
        /// ALS high threshold higher byte
        const THRESHOLD_AIHTH= 0x07;

        /// No Persist ALS low threshold lower byte
        const THRESHOLD_NPAILTL= 0x08;
        /// No Persist ALS low threshold higher byte
        const THRESHOLD_NPAILTH= 0x09;
        /// No Persist ALS high threshold lower byte
        const THRESHOLD_NPAIHTL= 0x0A;
        /// No Persist ALS high threshold higher byte
        const THRESHOLD_NPAIHTH= 0x0B;

        /// Interrupt persistence filter
        const PERSIST_FILTER= 0x0C;
        /// Package ID
        const PACKAGE_PID= 0x11;
        /// Device ID
        const DEVICE_ID= 0x12;
        /// Device internal status
        const DEVICE_STATUS= 0x13;

        /// channel 0 lower byte
        const CHAN0_LOW=  0x14;
        /// channel 0 higher byte
        const CHAN0_HIGH=  0x15;
        /// chan 1 lower byte
        const CHAN1_LOW= 0x16;
        /// chan 1 higher byte
        const CHAN1_HIGH= 0x17;
    }

    /// Function Commands / command & control flags
    #[repr(C)]
    struct CmdControlFlags: u8 {
        /// 1010 0000: bits 7 and 5 for 'command normal'
        const COMMAND_BIT= 0xA0;
        /// Special Function Command for "Clear ALS and no persist ALS interrupt"
        const CLEAR_INT= 0xE7;
        /// Special Function Command for "Interrupt set - forces an interrupt"
        const TEST_INT= 0xE4;
        /// read or write a word rather than a byte
        const WORD_BIT= 0x20;
        /// use block read/write
        const BLOCK_BIT= 0x20;
        const ENABLE_POWEROFF= 0x00;
        const ENABLE_POWERON= 0x01;
        /// ALS enable. Activates ALS function
        const ENABLE_AEN= 0x02;
        /// ALS interrupt enable. Allows ALS interrupts to be generated
        /// subject to the persist filter
        const ENABLE_AIEN=  0x10;
        /// No persist interrupt enable. NP threshold conditions will
        /// generate an interrupt & bypass persist filter
        const ENABLE_NPIEN= 0x80;
        // Get channel data cmd
        const CMD_CHAN0_LOW_DATA = Self::COMMAND_BIT.bits() | RegisterMap::CHAN0_LOW.bits();
        const CMD_CHAN0_HIGH_DATA = Self::COMMAND_BIT.bits() | RegisterMap::CHAN0_HIGH.bits();
        const CMD_CHAN1_LOW_DATA = Self::COMMAND_BIT.bits() | RegisterMap::CHAN1_LOW.bits();
        const CMD_CHAN1_HIGH_DATA = Self::COMMAND_BIT.bits() | RegisterMap::CHAN1_HIGH.bits();
        // Set cmd used for setting timing and gain
        const CMD_CTRL_SET = Self::COMMAND_BIT.bits() | RegisterMap::CONTROL.bits();
        const CMD_ENABLE_SET = Self::COMMAND_BIT.bits() | RegisterMap::ENABLE.bits();
        // device status
        const STATUS = Self::COMMAND_BIT.bits() | RegisterMap::DEVICE_STATUS.bits();

        const ENABLE_FLAGS = Self::ENABLE_POWERON.bits()
            | Self::ENABLE_AEN.bits()
            | Self::ENABLE_AIEN.bits()
            | Self::ENABLE_NPIEN.bits();
    }
}

/// i2c light sensor
pub struct TSL2591<I2C: I2c> {
    pub address: u8,
    pub i2c: I2C,
    gain: Gain,
    int_time: IntegrationTime,
    mode: Mode,
    delay: Delay,
    fault_count: u32,
}

impl<I2C: I2c> TSL2591<I2C> {
    const CMD_DISABLE: [u8; 2] = [
        CmdControlFlags::CMD_ENABLE_SET.bits(),
        CmdControlFlags::ENABLE_POWEROFF.bits(),
    ];

    const CMD_ENABLE: [u8; 2] = [
        CmdControlFlags::CMD_ENABLE_SET.bits(),
        CmdControlFlags::ENABLE_FLAGS.bits(),
    ];

    /// lux cooefficient
    const LUX_DF: f32 = 408.0;
    /*
    /// channel 0 coefficient
    const LUX_COEFB: f32 = 1.64;
    /// channel 1 coefficient A
    const LUX_COEFC: f32 = 0.59;
    /// channel 2 coefficient
    const LUX_COEFD: f32 = 0.86;
    */

    /// More consts for calculating lux
    const GAIN_LOW: f32 = 1.0;
    const GAIN_MEDIUM: f32 = 25.0;
    const GAIN_HIGH: f32 = 428.0;
    const GAIN_MAX: f32 = 9876.0;

    const IT100MS: f32 = 100.0;
    const IT200MS: f32 = 200.0;
    const IT300MS: f32 = 300.0;
    const IT400MS: f32 = 400.0;
    const IT500MS: f32 = 500.0;
    const IT600MS: f32 = 600.0;

    // TODO find reasonable value for this
    const FAULT_THRESHOLD: u32 = 5;

    pub fn new(i2c: I2C, address: u8, delay: Delay) -> Result<Self, I2cError> {
        let mut sensor = Self {
            i2c,
            address,
            delay,
            mode: Mode::default(),
            int_time: IntegrationTime::default(),
            gain: Gain::default(),
            fault_count: 0,
        };

        sensor.enable()?;
        sensor.set_timing(IntegrationTime::default())?;
        sensor.set_gain(Gain::default())?;

        Ok(sensor)
    }

    pub fn configure(
        &mut self,
        gain: Option<Gain>,
        int_time: Option<IntegrationTime>,
    ) -> Result<(), Tsl2591Error> {
        //   self.enable()?;
        if let Some(gain) = gain {
            self.set_gain(gain.clone())?;
            self.gain = gain;
        }
        if let Some(int_time) = int_time {
            self.set_timing(int_time.clone())?;
            self.int_time = int_time;
        }
        //    self.disable()?;
        Ok(())
    }

    pub fn enable(&mut self) -> Result<(), I2cError> {
        self.i2c
            .write(self.address, &Self::CMD_ENABLE)
            .map_err(|e| {
                log::error!("i2c write error (enable) {e:?}");
                I2cError::I2cWriteError
            })?;
        Ok(())
    }

    pub fn disable(&mut self) -> Result<(), I2cError> {
        self.i2c
            .write(self.address, &Self::CMD_DISABLE)
            .map_err(|e| {
                log::error!("i2c write error (disable) {e:?}");
                I2cError::I2cWriteError
            })?;
        Ok(())
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    fn set_gain(&mut self, gain: Gain) -> Result<(), I2cError> {
        // sensor.enable()?;
        self.i2c
            .write(
                self.address,
                &[
                    CmdControlFlags::CMD_CTRL_SET.bits(),
                    self.int_time.clone() as u8 | gain as u8,
                ],
            )
            .map_err(|e| {
                log::error!("i2c write error (gain) {e:?}");
                I2cError::I2cWriteError
            })?;
        // sensor.disable()?;
        Ok(())
    }

    fn set_timing(&mut self, int_time: IntegrationTime) -> Result<(), I2cError> {
        //sensor.enable()?;
        self.i2c
            .write(
                self.address,
                &[
                    CmdControlFlags::CMD_CTRL_SET.bits(),
                    int_time as u8 | self.gain.clone() as u8,
                ],
            )
            .map_err(|e| {
                log::error!("i2c write error (timing) {e:?}");
                I2cError::I2cWriteError
            })?;
        // sensor.disable()?;
        Ok(())
    }

    fn get_channel_data(&mut self) -> Result<u32, I2cError> {
        let mut data = [0; 4];
        self.i2c
            .write_read(
                self.address,
                &[CmdControlFlags::CMD_CHAN0_LOW_DATA.bits()],
                &mut data,
            )
            .map_err(|e| {
                log::error!("i2c write error (write/read get channel data) {e:?}");
                I2cError::I2cWriteReadError
            })?;
        Ok(u32::from(data[0])
            | (u32::from(data[1]) << 8)
            | (u32::from(data[2]) << 16)
            | (u32::from(data[3]) << 24))
    }

    fn get_luminosity(&mut self, channel: Mode) -> Result<u16, Tsl2591Error> {
        let full_luminosity = self.get_full_luminosity()?;

        // ir + visible
        let full: u16 = full_luminosity.bitand(0xffff) as u16;
        // ir
        let infra: u16 = full_luminosity.shr(16) as u16;

        match channel {
            Mode::Visible => {
                if full < infra {
                    self.fault_count += 1;
                    return Err(Tsl2591Error::SignalOverflow);
                }
                Ok(full - infra)
            }
            Mode::Infrared => Ok(infra),
            Mode::FullSpectrum => Ok(full),
        }
    }

    fn get_full_luminosity(&mut self) -> Result<u32, Tsl2591Error> {
        self.delay.delay_millis(120);
        Ok(self.get_channel_data()?)
    }

    /// https://github.com/adafruit/Adafruit_TSL2591_Library/blob/master/Adafruit_TSL2591.cpp#L225
    fn get_lux(&mut self) -> Result<f32, Tsl2591Error> {
        self.delay.delay_millis(120);
        let full_luminosity = self.get_channel_data()?;
        // ir
        let infra: u32 = full_luminosity.shr(16);
        let infra: f32 = infra.bitand(0xffff) as f32;
        // ir + visible
        let full = full_luminosity.bitand(0xffff) as f32;

        if (full as u16 == 0xffff) || (infra as u16 == 0xffff) {
            self.fault_count += 1;
            return Err(Tsl2591Error::SignalOverflow);
        }

        let time_factor = self.get_int_time_float();
        let gain_factor = self.get_gain_float();

        let check = (time_factor * gain_factor) / Self::LUX_DF;

        /*
        let lux1 = (full - (Self::LUX_COEFB * infra)) / check;
        let lux2 = ((Self::LUX_COEFC * full) - (Self::LUX_COEFD * infra)) / check;
        let lux = lux1.max(lux2);
        */
        if full < infra || full == 0.0 {
            self.fault_count += 1;
            return Err(Tsl2591Error::SignalOverflow);
        }

        let lux = (full - infra) * (1.0 - (infra / full)) / check;

        Ok(lux)
    }

    fn get_int_time_float(&self) -> f32 {
        match self.int_time {
            IntegrationTime::IntTime100 => Self::IT100MS,
            IntegrationTime::IntTime200 => Self::IT200MS,
            IntegrationTime::IntTime300 => Self::IT300MS,
            IntegrationTime::IntTime400 => Self::IT400MS,
            IntegrationTime::IntTime500 => Self::IT500MS,
            IntegrationTime::IntTime600 => Self::IT600MS,
        }
    }

    fn get_gain_float(&self) -> f32 {
        match self.gain {
            Gain::Low => Self::GAIN_LOW,
            Gain::Medium => Self::GAIN_MEDIUM,
            Gain::High => Self::GAIN_HIGH,
            Gain::Max => Self::GAIN_MAX,
        }
    }

    pub fn adjust_for_current_light(&mut self) -> Result<(), Tsl2591Error> {
        match self.gain {
            Gain::Low => self.adjust_for_mid_light()?,
            Gain::Medium => self.adjust_for_low_light()?,
            Gain::High => self.adjust_for_ultra_low_light()?,
            Gain::Max => self.adjust_for_bright_light()?,
        };
        self.fault_count = 0;
        Ok(())
    }

    pub fn adjust_for_bright_light(&mut self) -> Result<(), Tsl2591Error> {
        self.configure(Some(Gain::Low), Some(IntegrationTime::IntTime100))
    }

    pub fn adjust_for_mid_light(&mut self) -> Result<(), Tsl2591Error> {
        self.configure(Some(Gain::Medium), Some(IntegrationTime::IntTime300))
    }

    pub fn adjust_for_low_light(&mut self) -> Result<(), Tsl2591Error> {
        self.configure(Some(Gain::High), Some(IntegrationTime::IntTime500))
    }

    pub fn adjust_for_ultra_low_light(&mut self) -> Result<(), Tsl2591Error> {
        self.configure(Some(Gain::Max), Some(IntegrationTime::IntTime600))
    }

    fn fault_count_threshold(&self) -> bool {
        self.fault_count >= Self::FAULT_THRESHOLD
    }
}

#[allow(unused)]
impl<I2C> LightLumenSensor for TSL2591<I2C>
where
    I2C: I2c,
{
    fn luminosity(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, LightSensorError> {
        let mut local_start = start;
        let full_spec = self
            .get_luminosity(Mode::FullSpectrum)
            .map_err(LightSensorError::from)?;
        log::debug!("Full spectrum luminosity {:?}", full_spec);

        let size = core::mem::size_of::<u16>();
        buffer[start..start + size].copy_from_slice(&full_spec.to_le_bytes());
        Ok(size)
    }
}

#[allow(unused)]
impl<I2C> LightLuxSensor for TSL2591<I2C>
where
    I2C: I2c,
{
    fn lux(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, LightSensorError> {
        let reading = self.get_lux().map_err(LightSensorError::from)?;
        log::debug!("lux {:?}", reading);

        let size = core::mem::size_of::<f32>();
        buffer[start..start + size].copy_from_slice(&reading.to_le_bytes());
        Ok(size)
    }
}

impl<I2C> Sensor for TSL2591<I2C>
where
    I2C: I2c,
{
    fn read(&mut self, buffer: &mut [u8], start: usize) -> Result<usize, PlatformSensorError> {
        if self.fault_count_threshold() {
            log::warn!("Adjusting for consistent light sensor failures before attempting read");
            self.adjust_for_current_light()
                .map_err(LightSensorError::from)?;
        }
        let fs = self
            .get_luminosity(Mode::FullSpectrum)
            .map_err(LightSensorError::from)?;
        log::debug!("Full spectrum luminosity {:?}", fs);

        let lux = self.get_lux().map_err(LightSensorError::from)?;
        log::debug!("lux {:?}", lux);

        let reading: pmindp_sensor::Light = pmindp_sensor::Light { lux, fs };

        let reading = serde_json::to_vec(&reading).map_err(|e| {
            log::error!("Serde failed {e:}");
            PlatformSensorError::Other
        })?;
        let len = reading.len();

        buffer[start..start + len].copy_from_slice(&reading);

        Ok(len)
    }
}
