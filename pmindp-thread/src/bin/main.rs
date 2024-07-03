#![no_std]
#![no_main]

use core::cell::RefCell;
use core::pin::pin;

use critical_section::Mutex;
use esp_backtrace as _;

use esp_hal::{
    clock::ClockControl,
    delay::Delay,
    gpio::Io,
    i2c::I2C,
    peripherals::Peripherals,
    prelude::*,
    rng::Rng,
    system::SystemControl,
    timer::{
        systimer::SystemTimer,
        timg::{TimerGroup, TimerInterrupts},
    },
};
use esp_println::println;
use pmindp_thread::{led_setup, sensor_read, sensor_setup, SENSOR_TIMER_TG0_T0_LEVEL};

use esp_ieee802154::{Config, Ieee802154};
use esp_openthread::{NetworkInterfaceUnicastAddress, OperationalDataset, ThreadTimestamp};

use smart_leds::{brightness, colors, gamma, SmartLedsWrite};

pub const BOUND_PORT: u16 = 1212;

#[entry]
fn main() -> ! {
    //  esp_println::logger::init_logger(log::LevelFilter::Debug);

    let mut peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    let radio = peripherals.IEEE802154;
    let mut ieee802154 = Ieee802154::new(radio, &mut peripherals.RADIO_CLK);

    let mut openthread = esp_openthread::OpenThread::new(
        &mut ieee802154,
        systimer.alarm0,
        Rng::new(peripherals.RNG),
    );

    let changed = Mutex::new(RefCell::new(false));
    let mut callback = |flags| {
        println!("{:?}", flags);
        critical_section::with(|cs| *changed.borrow_ref_mut(cs) = true);
    };

    openthread
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

    openthread.set_change_callback(Some(&mut callback));

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
    println!("dataset : {:?}", dataset);

    openthread.set_active_dataset(dataset).unwrap();

    openthread.set_child_timeout(60).unwrap();

    openthread.ipv6_set_enabled(true).unwrap();

    openthread.thread_set_enabled(true).unwrap();

    let addrs: heapless::Vec<NetworkInterfaceUnicastAddress, 6> =
        openthread.ipv6_get_unicast_addresses();

    print_all_addresses(addrs);

    let mut socket = openthread.get_udp_socket::<512>().unwrap();
    let mut socket = pin!(socket);
    socket.bind(BOUND_PORT).unwrap();

    let mut buffer = [0u8; 512];

    let mut data;
    let mut eui: [u8; 6] = [0u8; 6];

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let led_pin = io.pins.gpio8;

    let mut led = led_setup(peripherals.RMT, led_pin, &clocks);

    let delay = Delay::new(&clocks);

    sensor_setup(
        &mut I2C::new(
            peripherals.I2C0,
            io.pins.gpio5,
            io.pins.gpio6,
            400.kHz(),
            &clocks,
            None,
        ),
        5000,
        TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            Some(TimerInterrupts {
                timer0: Some(SENSOR_TIMER_TG0_T0_LEVEL),
                ..Default::default()
            }),
        ),
    );

    let mut hello: Option<no_std_net::Ipv6Addr> = None;
    let mut send_data_buf: [u8; 6] = [0u8; 6];
    loop {
        openthread.process();
        openthread.run_tasklets();

        data = [colors::SEA_GREEN];
        led.write(brightness(gamma(data.iter().cloned()), 50))
            .unwrap();

        if let Ok(Some((moisture, temp))) = sensor_read(delay) {
            println!("Moisture: {:?}, temp: {:?}", moisture, temp);

            send_data_buf[..2].copy_from_slice(&moisture.to_le_bytes());
            send_data_buf[2..].copy_from_slice(&temp.to_be_bytes());

            if let Some(hello) = hello {
                socket.send(hello, BOUND_PORT, &send_data_buf).unwrap();
                data = [colors::MISTY_ROSE];
                led.write(brightness(gamma(data.iter().cloned()), 100))
                    .unwrap();
            }
        }

        let (len, from, port) = socket.receive(&mut buffer).unwrap();
        if len > 0 {
            println!(
                "received {:02x?} from {:?} port {}",
                &buffer[..len],
                from,
                port
            );

            if buffer[0] == 0xbe {
                println!("Beef face");
            }

            socket
                .send(from, BOUND_PORT, b"beefface authenticate!")
                .unwrap();
            hello = Some(from);
            println!("Handshake complete");
        }

        critical_section::with(|cs| {
            let mut c = changed.borrow_ref_mut(cs);
            if *c {
                let addrs: heapless::Vec<NetworkInterfaceUnicastAddress, 6> =
                    openthread.ipv6_get_unicast_addresses();

                print_all_addresses(addrs);
                let role = openthread.get_device_role();
                openthread.get_eui(&mut eui);
                println!("Role: {:?}, Eui {:#X?}", role, eui);
                *c = false;
            }
        });

        data = [colors::MEDIUM_ORCHID];
        led.write(brightness(gamma(data.iter().cloned()), 50))
            .unwrap();
    }
}

fn print_all_addresses(addrs: heapless::Vec<NetworkInterfaceUnicastAddress, 6>) {
    println!("Currently assigned addresses");
    for addr in addrs {
        println!("{}", addr.address);
    }
    println!();
}
