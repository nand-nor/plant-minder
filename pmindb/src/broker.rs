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
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    db::{CreateOrModify, DatabaseError, NodeSensorReading},
    monitor::{
        CheckNewNode, GetNodeStatus, MonitorNetworkStatus, OmrIp, Registration, ReserveFreePort,
        ReturnFreePort,
    },
    node::{NodeEvent, NodeHandler},
    Eui, OtCliClient, OtMonitor, OtMonitorError, PlantDatabaseHandler,
};

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
    db_registry_conn_handle: Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
    db_sensor_stream_conn_handle:
        Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
}

impl BrokerCoordinator {
    pub async fn new(path: std::path::PathBuf) -> Result<Self, BrokerCoordinatorError> {
        let (stream_tx, stream_rx) = unbounded_channel();
        let (registration_tx, registration_rx) = unbounded_channel();

        let mut broker = Self {
            monitor_handle: None,
            db_registry_conn_handle: None,
            db_sensor_stream_conn_handle: None,
        };

        // Initialize and start actors
        let database = PlantDatabaseHandler::new(path)?;
        let database_handle = database.start();
        let ot_mon = OtMonitor::new(Box::new(OtCliClient));
        let ot_mon_handle = ot_mon.start();

        broker
            .spawn_db_conn_registry_task(database_handle.clone(), registration_rx)
            .await;
        broker
            .spawn_child_mon_task(25, ot_mon_handle, stream_tx, registration_tx)
            .await;
        broker
            .spawn_db_conn_sensor_stream_task(database_handle, stream_rx)
            .await;

        Ok(broker)
    }

    pub async fn exec_task_loops(&mut self) {
        log::debug!("Starting event and monitor loop tasks...");
        self.db_registry_conn_handle.take().unwrap().await.ok();
        self.monitor_handle.take().unwrap().await.ok();
        self.db_sensor_stream_conn_handle.take().unwrap().await.ok();
    }

    async fn spawn_db_conn_sensor_stream_task(
        &mut self,
        db: Addr<PlantDatabaseHandler>,
        mut receiver: UnboundedReceiver<UnboundedReceiver<NodeEvent>>,
    ) {
        let handle = tokio::spawn(async move {
            loop {
                while let Some(rcv) = receiver.recv().await {
                    let db_clone = db.clone();
                    tokio::spawn(async move {
                        Self::process(UnboundedReceiverStream::new(rcv), db_clone).await
                    });
                }
            }
        });
        self.db_sensor_stream_conn_handle = Some(handle);
    }

    async fn process(
        mut stream: UnboundedReceiverStream<NodeEvent>,
        db: Addr<PlantDatabaseHandler>,
    ) {
        log::trace!("Processing NodeEvent receiver as a stream");
        while let Some(msg) = stream.next().await {
            let db_clone = db.clone();
            match msg {
                NodeEvent::NodeTimeout(addr) => {
                    log::warn!("Node {:?} timed out, closing receiver stream", addr);
                }
                NodeEvent::SensorReading(node) => {
                    log::debug!(
                        "Reading! from {:?} moisture {:?} temp {:?}",
                        node.addr,
                        node.data.moisture,
                        node.data.temperature
                    );

                    if let Err(e) = db_clone
                        .send(NodeSensorReading((*node.addr.ip(), node.data)))
                        .await
                    {
                        log::error!("Error sending to db handle {e:}");
                    }
                }
                NodeEvent::SocketError(addr) => {
                    log::warn!("Socket error on addr {:?}, closing receiver stream", addr);
                }
                event => {
                    log::warn!("Setup error {event:?}, closing receiver stream");
                }
            }
        }
        log::warn!("Stream processing func closing");
    }

    async fn coap_observer_register(
        omr_addr: Ipv6Addr,
        ip_addr: Ipv6Addr,
        port: u16,
    ) -> Result<Option<(SocketAddrV6, Eui)>, BrokerCoordinatorError> {
        log::info!("Starting CoAP Registration for {ip_addr:} on port {port:}");
        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        let mut buffer = [0u8; 512];
        // following https://datatracker.ietf.org/doc/html/rfc7641 observing resources in CoAP
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
                if let Ok(packet) = Packet::from_bytes(&buffer[..len]) {
                    let resp = CoapRequest::from_packet(packet, from);
                    if resp.message.payload.len() >= 6 {
                        eui.copy_from_slice(&resp.message.payload[..6]);
                    }
                }
                Ok(Some((addr, eui)))
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                Ok(None)
            }
        }
    }

    async fn spawn_db_conn_registry_task(
        &mut self,
        db: Addr<PlantDatabaseHandler>,
        mut registration_rcvr: UnboundedReceiver<(Eui, SocketAddrV6)>,
    ) {
        let handle = tokio::spawn(async move {
            while let Some((eui, rcv)) = registration_rcvr.recv().await {
                log::trace!("Node being added to DB {:?} addr {:?}", eui, rcv);
                if let Err(e) = db.send(CreateOrModify { eui, ip: *rcv.ip() }).await {
                    log::error!("database actor handle error {e:}");
                }
            }

            log::warn!("DB node registry task exiting");
            Ok(())
        });

        self.db_registry_conn_handle = Some(handle);
    }

    async fn spawn_child_mon_task(
        &mut self,
        poll_interval: u64,
        ot_mon: Addr<OtMonitor>,
        stream_sender: UnboundedSender<UnboundedReceiver<NodeEvent>>,
        registration_sender: UnboundedSender<(Eui, SocketAddrV6)>,
    ) {
        let poll = Duration::from_secs(poll_interval);
        let handle = tokio::spawn(async move {
            log::info!(
                "Setting up node / network monitor task to check every {:?} seconds",
                poll_interval
            );

            loop {
                log::debug!("Polling for network change");

                ot_mon
                    .send(MonitorNetworkStatus)
                    .await?
                    .map_err(|e| {
                        log::error!("Error checking omr prefix {e:}");
                        e
                    })
                    .ok();

                log::debug!("Polling for new nodes");
                // yuck, need better logic here
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
                                    let res = BrokerCoordinator::coap_observer_register(
                                        omr_addr, ip, free_port,
                                    )
                                    .await
                                    .map_err(|e| {
                                        log::error!("failure to register coap observer {e:}");
                                    });

                                    if let Ok(Some((addr, eui))) = res {
                                        // Update monitor registration record after successful CoAP reg
                                        ot_mon_clone
                                            .send(Registration {
                                                rloc,
                                                ip,
                                                eui,
                                                port: free_port,
                                            })
                                            .await
                                            .map_err(|e| log::error!("Failure to reg node {e:}"))
                                            .ok();

                                        let (sender, receiver) = unbounded_channel();

                                        // This object will spawn tasks that will not close unless there are appropriate
                                        // node events to trigger shutdown, such as node timeout, socket error, or
                                        // other lost node event
                                        let _new_node = NodeHandler::new(addr, sender).await;

                                        // Send the sensor data source to the task managing those streams
                                        if let Err(e) = _stream_sender.send(receiver) {
                                            // TODO
                                            log::error!("failure to send sensor stream {e:}");
                                        }

                                        // Send the sensor data source to the task managing those streams
                                        if let Err(e) = _registration_sender.send((eui, addr)) {
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

                log::debug!("Polling for lost nodes");
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

impl Drop for BrokerCoordinator {
    fn drop(&mut self) {
        if let Some(events) = &self.db_registry_conn_handle {
            events.abort();
        }
        if let Some(mon) = &self.monitor_handle {
            mon.abort();
        }
        if let Some(streams) = &self.db_sensor_stream_conn_handle {
            streams.abort();
        }
    }
}
