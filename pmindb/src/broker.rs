use actix::{Actor, MailboxError};
use coap_lite::{CoapRequest, ObserveOption, RequestType};
use futures::prelude::*;
use std::net::{Ipv6Addr, SocketAddr, SocketAddrV6};
use tokio::net::UdpSocket;
use tokio_stream::wrappers::UnboundedReceiverStream;

use actix::Addr;

use crate::{
    db::DatabaseError,
    monitor::{CheckNewNode, FreePort, GetNodeStatus, MonitorNetworkStatus, NodeRegistered, OmrIp},
    node::NodeHandler,
    OtCliClient, OtMonitor, OtMonitorError, PlantDatabase,
};

use crate::node::NodeEvent;
use pmindp_sensor::ATSAMD10SensorReading;
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

pub type Eui = [u8; 6];

pub struct BrokerCoordinator {
    monitor_handle: Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
    db_conn_handle: Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
    nodes_handle: Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
    sensor_stream_handle: Option<tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>>,
    //plant_db: PlantDatabase,

    //receiver: tokio::sync::mpsc::UnboundedReceiver<Addr<NodeHandler>>,
    // node_handles: Vec<Addr<NodeHandler>>,

    //socket: UdpSocket,
}

impl BrokerCoordinator {
    pub async fn new(path: std::path::PathBuf) -> Result<Self, BrokerCoordinatorError> {
        let ot_mon = OtMonitor::new(std::boxed::Box::new(OtCliClient));

        let (node_tx, node_rx) = tokio::sync::mpsc::unbounded_channel();
        let (stream_tx, stream_rx) = tokio::sync::mpsc::unbounded_channel();
        let (registration_tx, registration_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut broker = Self {
            monitor_handle: None,
            db_conn_handle: None,
            nodes_handle: None,
            sensor_stream_handle: None,
        };

        broker
            .spawn_db_conn_task(PlantDatabase::new(path)?, registration_rx)
            .await;
        broker.spawn_node_handling_task(node_rx).await;
        broker
            .spawn_child_mon_task(25, ot_mon, node_tx, stream_tx, registration_tx)
            .await;
        broker.spawn_sensor_streams_handle(stream_rx).await;

        Ok(broker)
    }

    pub async fn exec_task_loops(&mut self) {
        log::debug!("Starting event and monitor loop tasks...");
        self.db_conn_handle.take().unwrap().await.ok();
        self.monitor_handle.take().unwrap().await.ok();
        self.nodes_handle.take().unwrap().await.ok();
        self.sensor_stream_handle.take().unwrap().await.ok();
    }

    // TODO this will eventually pipe actors to the monitor task?
    async fn spawn_node_handling_task(
        &mut self,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<NodeHandler>,
        //mut sender: tokio::sync::mpsc::UnboundedSender<
        //     tokio::sync::mpsc::UnboundedReceiver<NodeEvent>,
        //>,
    ) {
        let handle = tokio::spawn(async move {
            let mut node_handles = vec![];
            loop {
                while let Some(mut node) = receiver.recv().await {
                    // TODO going to need to iterate through this and drop dead nodes at some point
                    node_handles.push(node);
                }
            }
        });
        self.nodes_handle = Some(handle);
    }

    async fn spawn_sensor_streams_handle(
        &mut self,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<
            tokio::sync::mpsc::UnboundedReceiver<NodeEvent>,
        >,
    ) {
        let handle = tokio::spawn(async move {
            loop {
                while let Some(mut rcv) = receiver.recv().await {
                    log::trace!("Received a NodeEvent receiver");
                    tokio::spawn(
                        async move { Self::process(UnboundedReceiverStream::new(rcv)).await },
                    );
                }
            }
        });
        self.sensor_stream_handle = Some(handle);
    }

    async fn process(mut stream: UnboundedReceiverStream<NodeEvent>) {
        log::trace!("Processing NodeEvent receiver as a stream");
        while let Some(msg) = stream.next().await {
            match msg {
                NodeEvent::NodeTimeout(addr) => {
                    // TODO signal the monitor handle to evict this entry
                    log::info!("Node {:?} timed out", addr);
                }
                NodeEvent::SensorReading(node) => {
                    // TODO hook this into something that exposes the data to a subscriber and/or database
                    log::info!(
                        "Reading! from {:?} moisture {:?} temp {:?}",
                        node.addr,
                        node.data.moisture,
                        node.data.temperature
                    );
                }
                NodeEvent::SocketError(addr) => {
                    // TODO signal the monitor handle to evict this entry
                    log::info!("Socket error on addr {:?}", addr);
                }
                _ => {
                    log::info!("Setup error");
                }
            }
        }
        log::info!("Stream processing func closing");
    }

    async fn coap_observer_register(
        omr_addr: Ipv6Addr,
        ip_addr: Ipv6Addr,
        port: u16,
    ) -> Result<Option<(SocketAddrV6, Eui)>, BrokerCoordinatorError> {
        log::info!("Registering {:?}", ip_addr);
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
                    // TODO its UDP so should we not use TCP for the initial handshake???
                    // need to look at CoAP spec
                    (len, from) = send_socket.recv_from(&mut buffer).await.map_err(|e|{
                        log::error!("Error receiving from socket: {e:}");
                        e
                    })?;
                }
                log::info!("Got a response from {from:}, expected {send_addr:}");

                let mut eui: Eui = [0u8; 6];
                if len >= 6 {
                    eui.copy_from_slice(&buffer[..6]);
                }
                Ok(Some((addr, eui)))
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                Ok(None)
            }
        }
    }

    async fn spawn_db_conn_task(
        &mut self,
        db: PlantDatabase,
        mut registration_rcvr: tokio::sync::mpsc::UnboundedReceiver<(Eui, SocketAddrV6)>,
    ) {
        let handle = tokio::spawn(async move {
            //   db.insert_reading(reading, sensor_id)
            loop {
                while let Some((eui, rcv)) = registration_rcvr.recv().await {
                    log::trace!("Received a NodeEvent receiver {:?} addr {:?}", eui, rcv);
                }
            }

            log::warn!("Socket listener task exiting");
            // to do signal to broker to retstart this task ?
            Ok(())
        });

        self.db_conn_handle = Some(handle);
    }

    async fn spawn_child_mon_task(
        &mut self,
        poll_interval: u64,
        ot_mon: OtMonitor,
        mut node_sender: tokio::sync::mpsc::UnboundedSender<NodeHandler>,
        mut stream_sender: tokio::sync::mpsc::UnboundedSender<
            tokio::sync::mpsc::UnboundedReceiver<NodeEvent>,
        >,
        mut registration_sender: tokio::sync::mpsc::UnboundedSender<(Eui, SocketAddrV6)>,
    ) {
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
                        let addr_clone = addr.clone();

                        futures::stream::iter(nodes)
                            .enumerate()
                            .for_each(|(i, (rloc, ip))| {
                                let addr_clone = addr.clone();
                                let mut _node_sender = node_sender.clone();
                                let mut _stream_sender = stream_sender.clone();
                                let mut _registration_sender = registration_sender.clone();

                                async move {
                                    let free_port: u16 = {
                                        if let Ok(Ok(free_port)) =
                                            addr_clone.clone().send(FreePort).await
                                        {
                                            free_port
                                        } else {
                                            // TODO pick some random number ?
                                            1213 + i as u16
                                        }
                                    };
                                    // TODO + rloc as u16 for port
                                    let res = BrokerCoordinator::coap_observer_register(
                                        omr_addr, ip, free_port,
                                    )
                                    .await
                                    .map_err(|e| {
                                        log::error!("failure to register coap observer {e:}");
                                    });

                                    if let Ok(Some((addr, eui))) = res {
                                        addr_clone
                                            .send(NodeRegistered((rloc, ip)))
                                            .await
                                            .map_err(|e| log::error!("Failure to reg node {e:}"))
                                            .ok();

                                        // TODO send receiver to monitor to get rid of stored addr??
                                        //  let (sender, receiver) =
                                        //    tokio::sync::mpsc::unbounded_channel();
                                        let (sender, receiver) =
                                            tokio::sync::mpsc::unbounded_channel();

                                        let new_node = NodeHandler::new(addr.clone(), sender).await;
                                        // new_node.handler.handler.await;
                                        // send the actor to the task managing those objects
                                        if let Err(e) = _node_sender.send(new_node) {
                                            // TODO
                                            log::error!("failure to send new node handler {e:}");
                                        }

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
        if let Some(events) = &self.db_conn_handle {
            events.abort();
        }
        if let Some(mon) = &self.monitor_handle {
            mon.abort();
        }
        if let Some(nodes) = &self.nodes_handle {
            nodes.abort();
        }

        if let Some(streams) = &self.sensor_stream_handle {
            streams.abort();
        }
    }
}
