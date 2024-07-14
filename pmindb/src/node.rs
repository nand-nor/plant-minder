/// Responsibilities:
/// 1. Open new socket to get sensor data from it (using the port sent in the registration)
/// 2. Notify broker when node stops sending data
/// 3. Stream sensor data to node event stream
use pmindp_sensor::ATSAMD10SensorReading;
use std::net::{SocketAddr, SocketAddrV6};
use tokio::{net::UdpSocket, sync::mpsc};

#[derive(Debug)]
pub enum NodeEvent {
    NodeTimeout(SocketAddrV6),
    SetupError,
    // TODO someday make this a dynamic trait object SensorReading
    // sp this can support different sensors
    SensorReading(NodeSensorReading),
}

#[derive(Debug)]
pub struct NodeSensorReading {
    pub addr: SocketAddr,
    pub data: ATSAMD10SensorReading,
}

pub struct NodeEventHandler {
    _handler: tokio::task::JoinHandle<()>,
}

const DEFAULT_TIMEOUT: u64 = 240;

impl NodeEventHandler {
    async fn new(addr: SocketAddrV6, sender: mpsc::UnboundedSender<NodeEvent>) -> Self {
        let _sender = sender.clone();
        let _handler = tokio::spawn(async move {
            let timeout = std::time::Duration::from_secs(DEFAULT_TIMEOUT);

            let sensor_read_socket = {
                if let Ok(sensor_read_socket) = UdpSocket::bind(addr.clone()).await {
                    sensor_read_socket
                } else {
                    _sender.send(NodeEvent::SetupError).ok();
                    return;
                }
            };

            let mut buffer = [0u8; 512];

            loop {
                let node_timeout = tokio::time::sleep(timeout);
                tokio::select! {
                  _ = _sender.closed() => {
                    log::error!("Sender is closed");
                    break;
                  }
                  _ = node_timeout => {
                    log::error!("Node timed out! No longer receiving data?");
                    _sender.send(NodeEvent::NodeTimeout(addr.clone())).ok();
                    break;
                  }
                  Ok((len,from)) = sensor_read_socket.recv_from(&mut buffer) => {
                        if len >= 6 {
                            let mut moisture_s: [u8; 2] = [0u8; 2];
                            moisture_s.copy_from_slice(&buffer[..2]);
                            let moisture = u16::from_le_bytes(moisture_s);
                            let mut temp_s: [u8; 4] = [0u8; 4];
                            temp_s.copy_from_slice(&buffer[2..6]);
                            let temperature = f32::from_le_bytes(temp_s);
                            // TODO error handling
                            _sender.send(NodeEvent::SensorReading(NodeSensorReading {
                                addr: from,
                                data: ATSAMD10SensorReading { moisture, temperature }
                            })
                            ).ok();
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
