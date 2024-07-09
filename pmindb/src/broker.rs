use actix::{Actor, MailboxError};
use coap_lite::{CoapRequest, ObserveOption, RequestType};
use futures::prelude::*;
use std::net::{Ipv6Addr, SocketAddr};
use tokio::net::UdpSocket;

use crate::{
    db::DatabaseError,
    monitor::{CheckNewNode, GetNodeStatus, MonitorNetworkStatus, NodeRegistered, OmrIp},
    OtCliClient, OtMonitor, OtMonitorError, PlantDatabase,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrokerCoordinatorError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("OT monitor Error")]
    OtMonError(#[from] OtMonitorError),
    #[error("Actix mailbox Error")]
    MailError(#[from] MailboxError),

    #[error("CoAP Msg Error")]
    CoAPMsgError(#[from] coap_lite::error::MessageError),

    #[error("AddrParse error")]
    AddrParse(#[from] std::net::AddrParseError),

    #[error("Database Error")]
    DatabaseError(#[from] DatabaseError),
}

pub struct BrokerCoordinator {
    monitor_handle: Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
    event_queue_handle: Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
    plant_db: PlantDatabase,
    //socket: UdpSocket,
}

impl BrokerCoordinator {
    pub async fn new(path: std::path::PathBuf) -> Result<Self, BrokerCoordinatorError> {
        let ot_mon = OtMonitor::new(std::boxed::Box::new(OtCliClient));

        let omr_addr = ot_mon.get_omr_ip()?;
        let addr = format!("[{}]:1212", omr_addr);
        let addr: std::net::SocketAddrV6 = addr.parse()?;

        let socket = UdpSocket::bind(addr).await?;

        let mut broker = Self {
            monitor_handle: None,
            event_queue_handle: None,
            plant_db: PlantDatabase::new(path)?,
        };

        broker.spawn_socket_listener(socket).await;
        broker.spawn_child_mon_task(25, ot_mon).await;

        Ok(broker)
    }

    pub async fn exec_task_loops(&mut self) {
        log::debug!("Starting event and monitor loop tasks...");
        self.event_queue_handle.take().unwrap().await.ok();
        self.monitor_handle.take().unwrap().await.ok();
    }

    pub async fn coap_observer_register(
        omr_addr: Ipv6Addr,
        ip_addr: Ipv6Addr,
        port: u16,
    ) -> Result<(), BrokerCoordinatorError> {
        log::info!("Registering {:?}", ip_addr);
        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        let mut buffer = [0u8; 512];
        // following https://datatracker.ietf.org/doc/html/rfc7641 observing resources in CoAP
        request.set_method(RequestType::Get);
        request.set_path("/soilmoisture");
        request.message.set_token(vec![0xfa, 0xce, 0xbe, 0xef]);
        request.set_observe_flag(ObserveOption::Register);
        let packet = request.message.to_bytes()?;

        let ip_w_port = format!("[{}]:1212", ip_addr);
        // fix this later
        let send_addr: std::net::SocketAddrV6 = ip_w_port.parse()?;

        let addr = format!("[{}]:{}", omr_addr, port);
        let addr: std::net::SocketAddrV6 = addr.parse()?;

        let send_socket = UdpSocket::bind(addr).await?;

        if let Err(e) = send_socket.send_to(&packet[..], send_addr).await {
            log::error!("Error sending: {e:}");
        }

        // allow retries in case the radio is currently idle
        // not currently enabling rx_on_when_idle, should only
        // be a couple seconds
        tokio::select! {
            Ok((mut len, mut from)) = send_socket.recv_from(&mut buffer) => {
                while len == 0 {
                    // sleep a lil
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    if let Err(e) = send_socket.send_to(&packet[..], send_addr).await {
                        log::error!("Error sending: {e:}");
                    }
                    // TODO its UDP so should we not use TCP for the initial handshake???
                    // need to look at CoAP spec
                    (len, from) = send_socket.recv_from(&mut buffer).await?
                }
                log::info!("Got a response from {from:}");

            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {

            }
        };

        Ok(())
    }

    pub async fn spawn_socket_listener(&mut self, socket: UdpSocket) {
        let handle = tokio::spawn(async move {
            let mut buffer = [0u8; 512];
            log::info!("Setting up listener on socket {:?}", socket);

            while let Ok((len, src)) = socket.recv_from(&mut buffer).await {
                if len > 0 {
                    let mut moisture_s: [u8; 2] = [0u8; 2];
                    moisture_s.copy_from_slice(&buffer[..2]);
                    let moisture = u16::from_le_bytes(moisture_s);
                    let mut temp_s: [u8; 4] = [0u8; 4];
                    temp_s.copy_from_slice(&buffer[2..6]);
                    let temp = f32::from_le_bytes(temp_s);

                    // TODO write these to an event queue
                    println!("{:?} sent moisture: {:?} temp {:?}", src, moisture, temp);
                }
            }
            log::warn!("Socket listener task exiting");
            // to do signal to broker to retstart this task ?
            Ok(())
        });

        self.event_queue_handle = Some(handle);
    }

    pub async fn spawn_child_mon_task(&mut self, poll_interval: u64, ot_mon: OtMonitor) {
        let addr = ot_mon.start();
        let poll = tokio::time::Duration::from_secs(poll_interval);
        let handle = tokio::spawn(async move {
            log::info!(
                "Setting up node / network monitor task to check every {:?} seconds",
                poll_interval
            );
            loop {
                log::debug!("Polling for network change");

                addr.send(MonitorNetworkStatus)
                    .await?
                    .map_err(|e| {
                        log::error!("Error checking omr prefix {e:}");
                        e
                    })
                    .ok();

                log::debug!("Polling for new nodes");
                // yuck, need better logic here
                if let Ok(nodes) = addr.send(CheckNewNode).await? {
                    if let Ok(omr_addr) = addr.send(OmrIp).await? {
                        futures::stream::iter(nodes)
                            .enumerate()
                            .for_each(|(i, (rloc, ip))| {
                                let addr_clone = addr.clone();
                                async move {
                                    // TODO + rloc as u16 for port
                                    let res = BrokerCoordinator::coap_observer_register(
                                        omr_addr,
                                        ip,
                                        1213 + i as u16,
                                    )
                                    .await
                                    .map_err(|e| {
                                        log::error!("failure to register coap observer {e:}");
                                    });

                                    if res.is_ok() {
                                        addr_clone
                                            .send(NodeRegistered((rloc, ip)))
                                            .await
                                            .map_err(|e| log::error!("Failure to reg node {e:}"))
                                            .ok();
                                    }
                                }
                            })
                            .await;
                    } else {
                        log::warn!("actor returned err on getting OmrIp");
                        // break;
                    }
                } else {
                    log::warn!("actor returned err on CheckNewNode");
                    // Break if we cant register new nodes
                    break;
                }

                log::debug!("Polling for lost nodes");
                if let Ok(lost_nodes) = addr.send(GetNodeStatus).await? {
                    // TODO need to handle this
                    if !lost_nodes.is_empty() {
                        log::warn!("Lost nodes {:?}", lost_nodes);
                    }
                } else {
                    log::warn!("actor returned err on GetNodeStatus");
                    // break;
                }
                tokio::time::sleep(poll).await;
            }

            log::warn!("Node / network monitor task exiting");
            // to do signal to broker to retstart this task ?

            //addr.terminate();
            //todo log somethin
            Ok(())
        });

        self.monitor_handle = Some(handle);
    }
}

impl Drop for BrokerCoordinator {
    fn drop(&mut self) {
        if let Some(events) = &self.event_queue_handle {
            events.abort();
        }
        if let Some(mon) = &self.monitor_handle {
            mon.abort();
        }
    }
}
