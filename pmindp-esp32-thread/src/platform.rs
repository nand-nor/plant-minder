use core::pin::pin;
use esp_hal::reset::software_reset_cpu;
use esp_openthread::{
    NetworkInterfaceUnicastAddress, OpenThread, OperationalDataset, ThreadTimestamp,
};

use coap_lite::{CoapRequest, Packet};
use pmindp_sensor::{PlatformSensorError, SensorPlatform};

use crate::{SensorVec, SENSOR_TIMER_FIRED};

// TODO put this in pmindp-sensor so other crates in
// workspace can access this
// also do that for any other constants that both
// sender and receiver need to know
pub const BOUND_PORT: u16 = 1212;

pub struct Esp32Platform<'a> {
    openthread: OpenThread<'a>,
    sensors: SensorVec,
}

pub enum Esp32PlatformError {
    SensorError,
    PlatformError,
    PeripheralError,
    OtherError,
}

impl<'a> Esp32Platform<'a>
where
    Esp32Platform<'a>: SensorPlatform,
{
    pub fn new(openthread: OpenThread<'a>, sensors: SensorVec) -> Self {
        Self {
            openthread,
            sensors,
        }
    }

    pub fn coap_server_event_loop(&mut self) -> Result<(), Esp32PlatformError> {
        self.openthread
            .set_radio_config(esp_ieee802154::Config {
                auto_ack_tx: true,
                auto_ack_rx: true,
                promiscuous: false,
                rx_when_idle: false,
                txpower: 18, // 18 txpower is legal for North America
                channel: 25, // match the dataset
                ..esp_ieee802154::Config::default()
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
        log::debug!("Programmed child device with dataset : {:?}", dataset);

        self.openthread.set_active_dataset(dataset).unwrap();
        self.openthread.ipv6_set_enabled(true).unwrap();
        self.openthread.thread_set_enabled(true).unwrap();

        let mut buffer = [0u8; 512];
        let mut eui: [u8; 6] = [0u8; 6];

        let mut observer_addr: Option<(no_std_net::Ipv6Addr, u16)> = None;
        // This block is needed to constrain how long the immutable borrow of openthread,
        // which happens when the socket object is created, exists
        {
            let mut socket = self.openthread.get_udp_socket::<512>().unwrap();
            let mut socket = pin!(socket);
            socket.bind(BOUND_PORT).unwrap();

            // make this big
            let mut send_data_buf: [u8; 127] = [0u8; 127];
            loop {
                self.openthread.process();
                self.openthread.run_tasklets();

                if let Some((observer, port)) = observer_addr {
                    let read_sensor = critical_section::with(|cs| {
                        let res = *SENSOR_TIMER_FIRED.borrow_ref_mut(cs);
                        *SENSOR_TIMER_FIRED.borrow_ref_mut(cs) = false;
                        res
                    });

                    if read_sensor {
                        match self.sensor_read(&mut send_data_buf) {
                            Ok(r) => {
                                if let Ok(sensor_data) = serde_json::to_vec(&r) {
                                    let len = sensor_data.len();
                                    if let Err(e) =
                                        socket.send(observer, port, &sensor_data[0..len])
                                    {
                                        // TODO depending on the error, need to set handshake IPv6 to None
                                        // until observer can reestablish conn; this will prevent the
                                        // node from sending data until success is better guaranteed
                                        log::error!("Error sending, resetting due to {e:?}");
                                        socket.close().ok();
                                        break;
                                    }
                                } else {
                                    log::error!("Unable to serialize sensor data");
                                }
                            }
                            Err(e) => {
                                log::error!("Sensor error, resetting due to {e:?}");
                                break;
                            }
                        };
                    }
                }

                let (len, from, port) = socket.receive(&mut buffer).unwrap();
                if len > 0 {
                    if let Ok(packet) = Packet::from_bytes(&buffer[..len]) {
                        let request = CoapRequest::from_packet(packet, from);

                        let plant_name = pmindp_sensor::PLANT_CONFIG.name;

                        let method = *request.get_method();
                        let path = request.get_path();
                        // TODO ! Need better solution
                        let port_req = request.message.header.message_id;

                        log::info!(
                            "Received CoAP request '{} {:?} {}' from {}",
                            port_req,
                            method,
                            path,
                            from
                        );

                        let mut response = request.response.unwrap();
                        self.openthread.get_eui(&mut eui);
                        let mut record = alloc::vec![];
                        record.extend_from_slice(&eui.clone());
                        record.extend_from_slice(plant_name.as_bytes());
                        response.message.payload = record.to_vec();
                        let packet = response.message.to_bytes().unwrap();
                        socket.send(from, port_req, packet.as_slice()).ok();

                        let addrs: heapless::Vec<NetworkInterfaceUnicastAddress, 6> =
                            self.openthread.ipv6_get_unicast_addresses();
                        print_all_addresses(addrs);

                        log::info!(
                            "Eui {:#X?} Plant Name {:?} Port {:?}",
                            eui,
                            plant_name,
                            port_req
                        );

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
            }
        }
        log::error!("Socket error, most likely node has dropped from the network");
        self.openthread.thread_set_enabled(false).unwrap();
        Err(Esp32PlatformError::PlatformError)
    }

    pub fn reset(&mut self) {
        software_reset_cpu();
    }
}

fn print_all_addresses(addrs: heapless::Vec<NetworkInterfaceUnicastAddress, 6>) {
    log::info!("Currently assigned addresses");
    for addr in addrs {
        log::info!("{}", addr.address);
    }
}

// TODO! Need to ensure that input buffer is long enough for the
// sensors to write to, since a variable number of sensors could be
// attached
impl<'a> SensorPlatform for Esp32Platform<'a> {
    fn sensor_read(
        &self,
        buffer: &mut [u8],
    ) -> Result<pmindp_sensor::SensorReading, PlatformSensorError> {
        let mut d = pmindp_sensor::SensorReading::default();

        let mut start = 0;
        self.sensors.iter().enumerate().for_each(|(idx, s)| {
            if let Some(s) = s {
                if let Ok(size) = critical_section::with(|cs| {
                    let mut sensor = s.borrow_ref_mut(cs);
                    let size = sensor.read(buffer, start)?;
                    Ok(size)
                })
                .map_err(|e: PlatformSensorError| {
                    log::error!("Error reading from sensor {e:?}");
                }) {
                    match idx {
                        pmindp_sensor::SOIL_IDX => {
                            if let Ok(soil_reading) =
                                serde_json::from_slice(&buffer[start..start + size]).map_err(|e| {
                                    log::error!("Unable to serialize soil reading {e:}");
                                    PlatformSensorError::Other
                                })
                            {
                                d.soil = soil_reading;
                            }
                        }
                        pmindp_sensor::LIGHT_IDX_1 => {
                            if let Ok(light_reading) =
                                serde_json::from_slice(&buffer[start..start + size])
                            {
                                d.light = Some(light_reading);
                            } else {
                                log::error!("Unable to serialize light reading");
                            }
                        }
                        pmindp_sensor::HUM_IDX => {
                            if let Ok(hum_reading) =
                                serde_json::from_slice(&buffer[start..start + size])
                            {
                                d.gas = Some(hum_reading);
                            } else {
                                log::error!("Unable to serialize humidity/gas reading");
                            }
                        }
                        // pmindp_sensor::LIGHT_IDX_2 =>{},
                        // pmindp_sensor::OTHER_IDX=>{},
                        _ => {
                            // all other types are currently unsupported
                        }
                    };

                    start += size;
                }
            }
        });
        d.ts = 0;
        log::info!("Sending {:?}", d);
        Ok(d)
    }
}
