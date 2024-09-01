use actix::{Actor, Addr, MailboxError};
use coap_lite::{CoapRequest, ObserveOption, Packet, RequestType};
use futures::prelude::*;
use std::{
    boxed::Box,
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
};
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::Duration,
};

use crate::{
    monitor::{
        CheckNewNode, GetNodeStatus, InternalRegistration, MonitorNetworkStatus, OmrIp,
        ReserveFreePort, ReturnFreePort,
    },
    node::{NodeEvent, NodeHandler},
    Eui, OtCliClient, OtMonitor, OtMonitorError,
};

#[derive(Error, Debug)]
pub enum EventRouterError {
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
}
pub struct EventRouter {
    monitor_handle: Option<tokio::task::JoinHandle<Result<(), EventRouterError>>>,
}

impl EventRouter {
    pub async fn new(
        stream_tx: UnboundedSender<UnboundedReceiver<NodeEvent>>,
        registration_tx: UnboundedSender<(Eui, Ipv6Addr, String)>,
        poll_interval: Duration,
    ) -> Result<Self, EventRouterError> {
        let mut broker = Self {
            monitor_handle: None,
        };

        let ot_mon = OtMonitor::new(Box::new(OtCliClient));
        let ot_mon_handle = ot_mon.start();

        broker
            .spawn_child_mon_task(poll_interval, ot_mon_handle, stream_tx, registration_tx)
            .await;

        Ok(broker)
    }

    pub async fn exec_monitor(&mut self) {
        self.monitor_handle.take().unwrap().await.ok();
    }

    async fn coap_observer_register(
        omr_addr: Ipv6Addr,
        ip_addr: Ipv6Addr,
        port: u16,
    ) -> Result<Option<(SocketAddrV6, Eui, Vec<u8>)>, EventRouterError> {
        log::info!("Starting CoAP Registration for {ip_addr:} on port {port:}");
        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        let mut buffer = [0u8; 512];
        // following https://datatracker.ietf.org/doc/html/rfc7641
        // observing resources in CoAP (loosely! needs work)
        request.set_method(RequestType::Get);
        request.set_path("/soilmoisture");
        request.message.set_token(vec![0xfa, 0xce, 0xbe, 0xef]);

        // TODO! Here we are using the message_id field to
        // tell the node what port we want to receive sensor data on
        request.message.header.message_id = port;
        request.set_observe_flag(ObserveOption::Register);
        let packet = request.message.to_bytes()?;

        let ip_w_port = format!("[{}]:1212", ip_addr);
        // fix this later
        let send_addr: SocketAddrV6 = ip_w_port.parse()?;

        let addr = format!("[{}]:{}", omr_addr, port);
        let addr: SocketAddrV6 = addr.parse()?;

        let send_socket = UdpSocket::bind(addr).await.map_err(|e| {
            log::error!("Unable to bind to socket at addr {:?}", addr);
            e
        })?;

        // Allow this to fail, there will be retries
        send_socket
            .send_to(&packet[..], send_addr)
            .await
            .map_err(|e| {
                log::error!("Error sending: {e:}");
            })
            .ok();

        // allow retries in case the radio is currently idle
        // not currently enabling rx_on_when_idle, should only
        // be a couple seconds
        tokio::select! {
            Ok((mut len, mut from)) = send_socket.recv_from(&mut buffer) => {
                while len == 0 {
                    // sleep a lil
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    send_socket.send_to(&packet[..], send_addr).await.map_err(|e|{
                        log::error!("Error sending: {e:}");
                    }).ok();

                    (len, from) = send_socket.recv_from(&mut buffer).await.map_err(|e|{
                        log::error!("Error receiving from socket: {e:}");
                        e
                    })?;
                }
                log::debug!("Got a response from {from:}, expected {send_addr:}");

                let mut eui: Eui = [0u8; 6];
                let mut name = vec![];
                if let Ok(packet) = Packet::from_bytes(&buffer[..len]) {
                    let resp = CoapRequest::from_packet(packet, from);
                    if resp.message.payload.len() >= 6 {
                        eui.copy_from_slice(&resp.message.payload[..6]);
                    }
                    name.clone_from(&resp.message.payload[6..].to_vec());
                }
                Ok(Some((addr, eui, name)))
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                Ok(None)
            }
        }
    }

    async fn spawn_child_mon_task(
        &mut self,
        poll: Duration,
        ot_mon: Addr<OtMonitor>,
        stream_sender: UnboundedSender<UnboundedReceiver<NodeEvent>>,
        registration_sender: UnboundedSender<(Eui, Ipv6Addr, String)>,
    ) {
        let handle = tokio::spawn(async move {
            log::info!(
                "Setting up node / network monitor task to check every {:?} seconds",
                poll
            );

            loop {
                log::info!(
                    "Monitor task: Polling for network change, new nodes, and missing nodes"
                );

                ot_mon
                    .send(MonitorNetworkStatus)
                    .await?
                    .map_err(|e| {
                        log::error!("Error checking omr prefix {e:}");
                        e
                    })
                    .ok();

                // TODO need serious refactor here
                if let Ok(nodes) = ot_mon.send(CheckNewNode).await? {
                    if let Ok(omr_addr) = ot_mon.send(OmrIp).await? {
                        futures::stream::iter(nodes)
                            .enumerate()
                            .for_each(|(i, (rloc, ip))| {
                                let ot_mon_clone = ot_mon.clone();
                                let mut _stream_sender = stream_sender.clone();
                                let mut _registration_sender = registration_sender.clone();

                                async move {
                                    // Get a free port from the monitor pool
                                    let free_port: u16 = {
                                        if let Ok(Ok(free_port)) =
                                            ot_mon_clone.clone().send(ReserveFreePort).await
                                        {
                                            free_port
                                        } else {
                                            // TODO pick some random number ?
                                            1213 + i as u16
                                        }
                                    };
                                    let res = EventRouter::coap_observer_register(
                                        omr_addr, ip, free_port,
                                    )
                                    .await
                                    .map_err(|e| {
                                        log::error!("failure to register coap observer {e:}");
                                    });

                                    if let Ok(Some((addr, eui, mut name))) = res {
                                        // Update monitor registration record after successful CoAP reg
                                        ot_mon_clone
                                            .send(InternalRegistration {
                                                rloc,
                                                ip,
                                                eui,
                                                port: free_port,
                                            })
                                            .await
                                            .map_err(|e| log::error!("Failure to reg node {e:}"))
                                            .ok();

                                        let (sender, receiver) = unbounded_channel();

                                        // This object will spawn tasks that will
                                        // not close unless there are appropriate
                                        // node events to trigger shutdown, such
                                        // as node timeout, socket error, or
                                        // other lost node event
                                        let _new_node = NodeHandler::new(addr, sender).await;

                                        // Send the sensor data source to the task
                                        // managing those streams
                                        if let Err(e) = _stream_sender.send(receiver) {
                                            // TODO
                                            log::error!("failure to send sensor stream {e:}");
                                        }

                                        // Shorten name (but this should be handled by
                                        // calling subscribers, so TODO move this)
                                        if name.len() > crate::MAX_PLANT_NAME_SIZE {
                                            name.drain(crate::MAX_PLANT_NAME_SIZE..);
                                        }

                                        // TODO: handle non utf8 input or in the case that
                                        // we have drained the vec to a size that is not a
                                        // valid codepoint?? can panic if we parse
                                        let name = String::from_utf8(name).unwrap_or_default();

                                        // Send the sensor data source to the task managing
                                        // those streams
                                        if let Err(e) = _registration_sender.send((eui, ip, name)) {
                                            // TODO
                                            log::error!("failure to send sensor stream {e:}");
                                        }
                                    } else {
                                        log::warn!("Registration failed, need to retry");
                                        ot_mon_clone.send(ReturnFreePort(free_port)).await.ok();
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

                if let Ok(lost_nodes) = ot_mon.send(GetNodeStatus).await? {
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
            Ok(())
        });

        self.monitor_handle = Some(handle);
    }
}

impl Drop for EventRouter {
    fn drop(&mut self) {
        if let Some(mon) = &self.monitor_handle {
            mon.abort();
        }
    }
}
