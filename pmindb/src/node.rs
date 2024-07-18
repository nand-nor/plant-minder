/// Responsibilities:
/// 1. Open new socket to get sensor data from it (using the port sent in the registration)
/// 2. Notify broker when node stops sending data
/// 3. Stream sensor data to node event stream
use pmindp_sensor::ATSAMD10SensorReading;
use std::net::{Ipv6Addr, SocketAddr, SocketAddrV6};
use tokio::{net::UdpSocket, sync::mpsc};

#[derive(Debug, Clone, Copy)]
pub enum NodeEvent {
    NodeTimeout(SocketAddrV6),
    SocketError(SocketAddrV6),
    SetupError,
    // TODO someday make this a dynamic trait object SensorReading
    // sp this can support different sensors
    SensorReading(NodeSensorReading),
}

pub type Registration = (crate::Eui, Ipv6Addr);

#[derive(Debug, Clone, Copy)]
pub struct NodeSensorReading {
    pub addr: SocketAddrV6,
    pub data: ATSAMD10SensorReading,
}

pub struct NodeEventHandler {
    _handler: tokio::task::JoinHandle<()>,
}

const DEFAULT_TIMEOUT: u64 = 100;

impl NodeEventHandler {
    async fn new(addr: SocketAddrV6, sender: mpsc::UnboundedSender<NodeEvent>) -> Self {
        let _sender = sender.clone();
        let _handler = tokio::spawn(async move {
            let timeout = std::time::Duration::from_secs(DEFAULT_TIMEOUT);

            let sensor_read_socket = {
                if let Ok(sensor_read_socket) = UdpSocket::bind(addr).await {
                    sensor_read_socket
                } else {
                    log::error!("Unable to bind to socket {addr:}");
                    _sender.send(NodeEvent::SetupError).ok();
                    return;
                }
            };

            let mut buffer = [0u8; 512];
            // Update this in the socket poll loop to make sure that when we report
            // a node event (error) with an address, we return the actual node ip not the
            // addr we used to open a socket
            let mut node_addr = addr;
            loop {
                let node_timeout = tokio::time::sleep(timeout);
                tokio::select! {
                  _ = _sender.closed() => {
                    log::error!("Sender is closed");
                    break;
                  }
                  _ = node_timeout => {
                    log::error!("Node timed out! No longer receiving data?");
                    _sender.send(NodeEvent::NodeTimeout(node_addr)).ok();
                    drop(sensor_read_socket);
                    break;
                  }
                  res = sensor_read_socket.recv_from(&mut buffer) => {
                        match res {
                            Ok((len, from)) => {
                                // This should always be true unless we get some bad actor sending
                                // us non-ipv6 traffic at this port
                                if let SocketAddr::V6(a) = from {
                                    node_addr = a;
                                } else {
                                    log::warn!("Non-ipv6 address sent data to our sensor port {from:}");
                                }
                                if len >= 6 {
                                    let mut moisture_s: [u8; 2] = [0u8; 2];
                                    moisture_s.copy_from_slice(&buffer[..2]);
                                    let moisture = u16::from_le_bytes(moisture_s);
                                    let mut temp_s: [u8; 4] = [0u8; 4];
                                    temp_s.copy_from_slice(&buffer[2..6]);
                                    let temperature = f32::from_le_bytes(temp_s);
                                    // TODO error handling
                                    _sender.send(NodeEvent::SensorReading(NodeSensorReading {
                                        addr: node_addr,
                                        data: ATSAMD10SensorReading { moisture, temperature }
                                    })
                                    ).ok();
                                }

                            }
                            _ => {
                                log::error!("Socket error");
                                _sender.send(NodeEvent::SocketError(node_addr)).ok();
                                drop(sensor_read_socket);
                                break;
                            }
                        }
                    }
                };
            }
        });

        Self { _handler }
    }
}

pub struct NodeHandler {
    _handler: NodeEventHandler,
}

impl NodeHandler {
    pub async fn new(addr: SocketAddrV6, sender: mpsc::UnboundedSender<NodeEvent>) -> Self {
        Self {
            _handler: NodeEventHandler::new(addr, sender).await,
        }
    }
}
