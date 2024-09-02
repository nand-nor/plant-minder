use chrono::Local;
use pmindp_sensor::SensorReading;
use std::net::{SocketAddr, SocketAddrV6};
use tokio::{net::UdpSocket, sync::mpsc};

use crate::Registration;

#[derive(Debug, Clone, Copy)]
pub enum NodeEvent {
    NodeTimeout(SocketAddrV6),
    SocketError(SocketAddrV6),
    SetupError,
    // TODO someday make this a dynamic trait object SensorReading
    // sp this can support different sensors
    SensorReading(NodeSensorReading),
}

/// [`NodeState`] can be used by client subscribers to create a
/// state machine that tracks when nodes are online, offline, or
/// unknown (to render appropriately in front end as needed)
#[derive(Debug, Clone, Copy)]
pub enum NodeState {
    Unknown,
    Offline(ErrorState),
    Online,
}

/// [`NodeStatus`] is used to separate out data receipt events
/// from registration or node fall-off when routing to
/// client subscribers
#[derive(Debug, Clone)]
pub enum NodeStatus {
    Registration(Registration),
    Termination((SocketAddrV6, ErrorState)),
}

/// [`ErrorState`] is reported to client subscribers via
/// [`NodeStatus::Termination`] and is meant to be used to
/// transition node state (as reflected via [`NodeState`] )
/// as needed
#[derive(Debug, Clone, Copy)]
pub enum ErrorState {
    Timeout,
    SocketError,
    SetupError,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub struct NodeSensorReading {
    pub addr: SocketAddrV6,
    pub data: SensorReading,
}

/// [`NodeEventHandler`] handles all events pertaining to child nodes on the
/// Thread mesh that support reporting sensor data. All such node events are
/// condensed into a single enum, [`NodeEvent`], which is split out into
/// state events (node registration, node termination) and data report events
/// in the [`Broker`](`crate::broker::Broker`) layer, once it has been routed
/// via the [`EventRouter`](`crate::router::EventRouter`).
///
/// [`NodeEventHandler`] has the following responsibilities:
/// 1. Open new socket to start receiving sensor data (using the port sent in
/// the CoAP registration)
/// 2. Track state of socket and time since last socket activity, in order to
/// notify [`Broker`](`crate::broker::Broker`) when node stops sending data, and
/// indicate the reason (e.g. due to timeout or socket error) as [`ErrorState`]
/// 3. Stream sensor data to node event stream as it is received on the socket
/// which gets routed via the [`EventRouter`](`crate::router::EventRouter`) to the
/// event queue exposed to client subscribers by the
/// [`Broker`](`crate::broker::Broker`)
pub struct NodeEventHandler {
    _handler: tokio::task::JoinHandle<()>,
}

impl NodeEventHandler {
    async fn new(addr: SocketAddrV6, sender: mpsc::UnboundedSender<NodeEvent>) -> Self {
        let _sender = sender.clone();
        let _handler = tokio::spawn(async move {
            let timeout = std::time::Duration::from_secs(crate::DEFAULT_TIMEOUT);

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
                                    log::warn!("Non-ipv6 address sent {from:}");
                                }

                                if let Ok(mut data) =
                                    serde_json::from_slice::<SensorReading>(
                                        &buffer[..len]
                                    ).map_err(|e|{
                                        log::error!("Deserde error {e:} len {:?}", len);
                                    }) {
                                        log::trace!("got data from node {:?}", data);
                                        data.ts = Local::now().timestamp();
                                        _sender.send(
                                            NodeEvent::SensorReading(
                                                NodeSensorReading {
                                                    addr: node_addr,
                                                    data,
                                                }
                                            )
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
