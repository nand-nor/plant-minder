//! Build for remote sensor setup using espressif 802154
//! capable dev boards (Currently only esp32-c6 and esp32-h2)
//! Use the espflash toolchain to build / flash / monitor

#![no_std]

extern crate alloc;

//mod sensor;

use core::{cell::RefCell, pin::pin, ptr::addr_of_mut};

use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};

use esp_hal::{
    clock::Clocks,
    delay::Delay,
    gpio::GpioPin,
    i2c::I2C,
    interrupt::{self, Priority},
    peripheral::Peripheral,
    peripherals::{Interrupt, I2C0, TIMG0},
    peripherals::{RMT, RNG},
    prelude::*,
    reset::software_reset_cpu,
    rmt::{Channel, Rmt},
    rng::Rng,
    timer::systimer::{Alarm, Target},
    timer::timg::{Timer, Timer0, TimerGroup},
    Blocking,
};

use pmindp_sensor::ATSAMD10;

use critical_section::Mutex;
use esp_ieee802154::{Config, Ieee802154};
use esp_openthread::{
    NetworkInterfaceUnicastAddress, OpenThread, OperationalDataset, ThreadDeviceRole,
    ThreadTimestamp,
};

use coap_lite::{CoapRequest, Packet};
use core::borrow::BorrowMut;
use pmindp_sensor::SoilSensorError;
use smart_leds::{brightness, colors, gamma, SmartLedsWrite};

pub const BOUND_PORT: u16 = 1212;

#[global_allocator]
static ALLOC: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

pub fn init_heap() {
    const SIZE: usize = 32768;
    static mut HEAP: [u8; SIZE] = [0; SIZE];
    unsafe { ALLOC.init(addr_of_mut!(HEAP) as *mut u8, SIZE) }
}

pub struct Esp32Platform<'a> {
    led: SmartLedsAdapter<Channel<Blocking, 0>, 25>,
    sensor: Mutex<RefCell<ATSAMD10<I2C<'a, I2C0, Blocking>>>>,
    openthread: OpenThread<'a>,
    delay: Delay,
}

pub enum Esp32PlatformError {
    SensorError,
    PlatformError,
    PeripheralError,
    OtherError,
}

static SENSOR_TIMER: Mutex<RefCell<Option<Timer<Timer0<TIMG0>, esp_hal::Blocking>>>> =
    Mutex::new(RefCell::new(None));

const DEFAULT_MIN_INTERVAL: u64 = 5000;

static SENSOR_TIMER_INTERVAL: Mutex<RefCell<u64>> = Mutex::new(RefCell::new(DEFAULT_MIN_INTERVAL));

static SENSOR_TIMER_FIRED: Mutex<RefCell<bool>> = Mutex::new(RefCell::new(false));

impl<'a> Esp32Platform<'a> {
    pub fn new(
        ieee802154: &'a mut Ieee802154,
        clocks: &Clocks,
        systimer: Alarm<Target, Blocking, 0>,
        i2c: impl Peripheral<P = I2C0> + 'a,
        timg0: TimerGroup<TIMG0, Blocking>,
        rmt: impl Peripheral<P = RMT> + 'a,
        led_pin: GpioPin<8>,
        sda_pin: GpioPin<5>,
        scl_pin: GpioPin<6>,
        rng: RNG,
    ) -> Self {
        let openthread = esp_openthread::OpenThread::new(ieee802154, systimer, Rng::new(rng));
        #[cfg(not(feature = "esp32h2"))]
        let rmt = Rmt::new(rmt, 80.MHz(), clocks, None).unwrap();
        #[cfg(feature = "esp32h2")]
        let rmt = Rmt::new(rmt, 32.MHz(), &clocks, None).unwrap();

        let rmt_buffer = smartLedBuffer!(1);
        let led = SmartLedsAdapter::new(rmt.channel0, led_pin, rmt_buffer, clocks);

        let timer = timg0.timer0;
        setup_sensor_timer(timer, 25000);

        // Read / Write / methods for pulling moisture and temp are defined in
        // pmindp-sensor
        let sensor = ATSAMD10 {
            i2c: I2C::new(
                i2c,     //peripherals.I2C0,
                sda_pin, //io.pins.gpio5,
                scl_pin, //io.pins.gpio6,
                400.kHz(),
                clocks,
                None,
            ),
            temp_delay: 2000,
            moisture_delay: 5000,
            address: 0x36,
        };

        Self {
            led,
            openthread,
            sensor: Mutex::new(RefCell::new(sensor)),
            delay: Delay::new(&clocks),
        }
    }

    pub fn sensor_read(&self) -> Result<Option<(u16, f32)>, SoilSensorError> {
        let read_sensor = critical_section::with(|cs| {
            let res = *SENSOR_TIMER_FIRED.borrow_ref_mut(cs);
            *SENSOR_TIMER_FIRED.borrow_ref_mut(cs) = false;
            res
        });

        if read_sensor {
            let res = critical_section::with(|cs| {
                let mut i2c = self.sensor.borrow_ref_mut(cs);
                let i2c = i2c.borrow_mut();
                let m_delay = i2c.moisture_delay;
                let t_delay = i2c.temp_delay;
                let moisture = i2c.moisture(|_| self.delay.delay_micros(m_delay))?;
                let temp = i2c.temperature(|_| self.delay.delay_micros(t_delay))?;
                Ok(Some((moisture, temp)))
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

    pub fn coap_server_event_loop(&mut self) -> Result<(), Esp32PlatformError> {
        self.openthread
            .set_radio_config(Config {
                auto_ack_tx: true,
                auto_ack_rx: true,
                promiscuous: false,
                rx_when_idle: false,
                txpower: 18, // 18 txpower is legal for North America
                channel: 25, // match the dataset
                ..Config::default()
            })
            .unwrap();

        let dataset = OperationalDataset {
            active_timestamp: Some(ThreadTimestamp {
                seconds: 1,
                ticks: 0,
                authoritative: false,
            }),
            network_key: Some([
                0xfe, 0x04, 0x58, 0xf7, 0xdb, 0x96, 0x35, 0x4e, 0xaa, 0x60, 0x41, 0xb8, 0x80, 0xea,
                0x9c, 0x0f,
            ]),
            network_name: Some("OpenThread-58d1".try_into().unwrap()),
            extended_pan_id: Some([0x3a, 0x90, 0xe3, 0xa3, 0x19, 0xa9, 0x04, 0x94]),
            pan_id: Some(0x58d1),
            channel: Some(25),
            channel_mask: Some(0x07fff800),

            ..OperationalDataset::default()
        };
        log::info!("dataset : {:?}", dataset);

        self.openthread.set_active_dataset(dataset).unwrap();
        self.openthread.set_child_timeout(60).unwrap();
        self.openthread.ipv6_set_enabled(true).unwrap();
        self.openthread.thread_set_enabled(true).unwrap();

        let mut buffer = [0u8; 512];

        let mut data;
        let mut eui: [u8; 6] = [0u8; 6];

        let mut observer_addr: Option<(no_std_net::Ipv6Addr, u16)> = None;
        // This block is needed to constrain how long the immutable borrow of openthread,
        // which happens when the socket object is created, exists
        {
            let mut socket = self.openthread.get_udp_socket::<512>().unwrap();
            let mut socket = pin!(socket);
            socket.bind(BOUND_PORT).unwrap();

            let mut send_data_buf: [u8; 6] = [0u8; 6];
            loop {
                self.openthread.process();
                self.openthread.run_tasklets();

                data = [colors::SEA_GREEN];
                self.led
                    .write(brightness(gamma(data.iter().cloned()), 50))
                    .unwrap();

                if let Some((observer, port)) = observer_addr {
                    if let Ok(Some((moisture, temp))) = self.sensor_read() {
                        log::info!("Moisture: {:?}, temp: {:?}", moisture, temp);

                        send_data_buf[..2].copy_from_slice(&moisture.to_le_bytes());
                        send_data_buf[2..].copy_from_slice(&temp.to_le_bytes());

                        let role = self.openthread.get_device_role();
                        log::info!("Role: {:?}", role);

                        match role {
                            ThreadDeviceRole::Detached
                            | ThreadDeviceRole::Unknown
                            | ThreadDeviceRole::Disabled => {}
                            _ => {
                                if let Err(e) = socket.send(observer, port, &send_data_buf) {
                                    // TODO depending on the error, need to set handshake IPv6 to None
                                    // until observer can reestablish conn; this will prevent the
                                    // node from sending data until success is better guaranteed
                                    log::info!(
                                        "Error sending, first print all ips??? then reset due to {:?}",
                                        e
                                    );
                                    socket.close().ok();
                                    break;
                                } else {
                                    data = [colors::MISTY_ROSE];
                                    self.led
                                        .write(brightness(gamma(data.iter().cloned()), 100))
                                        .ok();
                                }
                            }
                        };
                    }
                }

                let (len, from, port) = socket.receive(&mut buffer).unwrap();
                if len > 0 {
                    if let Ok(packet) = Packet::from_bytes(&buffer[..len]) {
                        let request = CoapRequest::from_packet(packet, from);

                        let method = request.get_method().clone();
                        let path = request.get_path();
                        // TODO ! Need better solution
                        let port_req = request.message.header.message_id;
                        //let token = request.get_token();
                        log::info!(
                            "Received CoAP request '{} {:?} {}' from {}",
                            port_req,
                            method,
                            path,
                            from
                        );

                        let mut response = request.response.unwrap();
                        self.openthread.get_eui(&mut eui);
                        response.message.payload = eui.to_vec();

                        //response.message.payload = b"beefface authenticate!".to_vec();

                        let packet = response.message.to_bytes().unwrap();
                        socket.send(from, port_req, packet.as_slice()).ok();

                        let addrs: heapless::Vec<NetworkInterfaceUnicastAddress, 6> =
                            self.openthread.ipv6_get_unicast_addresses();
                        print_all_addresses(addrs);
                        let role = self.openthread.get_device_role();
                        log::info!("Role: {:?}, Eui {:#X?} port {:?}", role, eui, port_req);

                        drop(packet);

                        observer_addr = Some((from, port_req));
                        log::info!("Handshake complete");
                    } else {
                        log::info!(
                            "received {:02x?} from {:?} port {}",
                            &buffer[..len],
                            from,
                            port
                        );

                        socket
                            .send(from, BOUND_PORT, b"beefface authenticate!")
                            .unwrap();
                    }
                }

                data = [colors::MEDIUM_ORCHID];
                self.led
                    .write(brightness(gamma(data.iter().cloned()), 50))
                    .unwrap();
            }
            // Drop the socket
            drop(socket);
        }
        log::error!("Socket error, most likely node has dropped from the network");
        self.openthread.thread_set_enabled(false).unwrap();
        Err(Esp32PlatformError::PlatformError)
    }

    pub fn reset(&mut self) {
        //software_reset_cpu();
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

fn print_all_addresses(addrs: heapless::Vec<NetworkInterfaceUnicastAddress, 6>) {
    log::info!("Currently assigned addresses");
    for addr in addrs {
        log::info!("{}", addr.address);
    }
}
