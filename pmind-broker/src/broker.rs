use actix::{prelude::*, Actor, Addr};
use futures::prelude::*;
use std::{collections::HashMap, net::SocketAddrV6};
use thiserror::Error;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::Duration,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    ClientId, ErrorState, EventRouter, EventRouterError, NodeEvent, NodeSensorReading, NodeStatus,
    Registration,
};

#[derive(Error, Debug)]
pub enum BrokerError {
    #[error("BrokerError")]
    Broker(#[from] EventRouterError),
    #[error("ActorError")]
    ActorError,
}

pub struct Broker {
    pub data_queue: UnboundedSender<NodeSensorReading>,
    pub data_queue_rx: UnboundedReceiver<NodeSensorReading>,
    sender: UnboundedSender<BrokerEvent>,
    receiver: UnboundedReceiver<BrokerEvent>,
    _event_handler: tokio::task::JoinHandle<()>,
    subscribers: HashMap<
        ClientId,
        (
            UnboundedSender<NodeSensorReading>,
            UnboundedSender<NodeStatus>,
        ),
    >,
    subscription_receiver: UnboundedReceiver<ClientApi>,
}

/// [`BrokerEvent`] enum is used by both the Broker and the [`EventRouter`] to
/// route events and data from received socket into the event queue that
/// the Broker exposed to subscribed clients
#[derive(Debug)]
pub enum BrokerEvent {
    NodeRegistration(Registration),
    NodeTermination((SocketAddrV6, ErrorState)),
    SensorReportHandleCreate(UnboundedReceiver<NodeEvent>),
}

/// The [`BrokerHandle`] provides clients a minimal handle exposing only the
/// client subscription API of the [`Broker`]. This handle enables
/// subscription to events and data by the client (as well as unsubscribing)
pub struct BrokerHandle(UnboundedSender<ClientApi>);

pub enum ClientApi {
    Subscribe {
        id: ClientId,
        sensor_readings: UnboundedSender<NodeSensorReading>,
        node_status: UnboundedSender<NodeStatus>,
    },
    Unsubscribe {
        id: ClientId,
    },
}

/// Public client API for instantiating a [`Broker`]. Returns to the caller a
///  [`BrokerHandle`] with which the client can subscribe or unsibscribe via
///  [`ClientApi`]
pub async fn broker(
    poll_interval: Duration,
    tick_rate_millis: u64,
) -> Result<Addr<BrokerHandle>, BrokerError> {
    let (stream_tx, stream_rx) = unbounded_channel();
    let (registration_tx, registration_rx) = unbounded_channel();

    let mut event_router = EventRouter::new(stream_tx, registration_tx, poll_interval).await?;

    tokio::spawn(async move {
        event_router.exec_monitor().await;
    });

    let (mut broker, handle) = Broker::new(tick_rate_millis, stream_rx, registration_rx).await;

    tokio::spawn(async move {
        broker.event_loop().await;
        log::warn!("Broker exiting event loop");
    });
    let handle = handle.start();

    Ok(handle)
}

impl Broker {
    async fn new(
        tick_rate_millis: u64,
        node_data_rx: UnboundedReceiver<UnboundedReceiver<NodeEvent>>,
        node_reg_rx: UnboundedReceiver<Registration>,
    ) -> (Self, BrokerHandle) {
        let tick_rate = Duration::from_millis(tick_rate_millis);
        let (sender, receiver) = unbounded_channel();
        let _sender = sender.clone();

        let mut node_event_stream = UnboundedReceiverStream::new(node_data_rx);
        let mut node_reg_stream = UnboundedReceiverStream::new(node_reg_rx);

        let (handle_sender, subscription_receiver) = unbounded_channel();
        let broker_handle = BrokerHandle(handle_sender);

        let _event_handler = tokio::spawn(async move {
            let mut tick = tokio::time::interval(tick_rate);

            loop {
                let tick_delay = tick.tick();
                let node_event_stream = node_event_stream.next().fuse();
                let node_reg_stream = node_reg_stream.next().fuse();

                tokio::select! {
                  _ = _sender.closed() => {
                    break;
                  }
                  _ = tick_delay => {}
                  Some(node_evt) = node_event_stream => {
                    log::trace!("node event {node_evt:?}");
                    _sender.send(BrokerEvent::SensorReportHandleCreate(node_evt)).ok();

                  }
                  Some(reg) = node_reg_stream => {
                    log::trace!("Node registration {reg:?}");
                    _sender.send(BrokerEvent::NodeRegistration(reg)).ok();
                  }
                };
            }
        });

        let (data_queue, data_queue_rx) = unbounded_channel();

        (
            Self {
                sender,
                receiver,
                _event_handler,
                data_queue,
                data_queue_rx,
                subscribers: HashMap::new(),
                subscription_receiver,
            },
            broker_handle,
        )
    }

    async fn event_loop(&mut self) {
        loop {
            tokio::select! {
                Some(incoming) = self.receiver.recv() => {
                    match incoming {
                        BrokerEvent::NodeRegistration(reg) => {
                            self.subscribers.iter().for_each(|(key, val)|{
                                val.1.send(
                                    NodeStatus::Registration(reg.clone())).map_err(|e|{
                                        log::error!("Failure to send to client event \
                                            receiver {e:} for client ID {key:}");
                                    }
                                ).ok();
                            });
                        }
                        BrokerEvent::NodeTermination((addr, state)) => {
                            self.subscribers.iter().for_each(|(key, val)|{
                                val.1.send(
                                    NodeStatus::Termination((addr, state))).map_err(|e|{
                                        log::error!("Failure to send to client event \
                                            receiver {e:} for client ID {key:}");
                                    }
                                ).ok();
                            });

                        },
                        BrokerEvent::SensorReportHandleCreate(rcv) => {
                            self.handle_sensor_stream_task(rcv).await
                        }
                    };
                }
                Some(data) = self.data_queue_rx.recv() => {
                    self.subscribers.iter().for_each(|(_key, val)|{
                        val.0.send(data).map_err(|e|{
                            log::error!("Failure to send to client data \
                                receiver {e:} for client ID {key:}");
                        }).ok();
                    });
                }
                Some(msg) = self.subscription_receiver.recv() => {
                    match msg {
                        ClientApi::Subscribe { id, sensor_readings, node_status } => {
                            self.subscribers.insert(id, (sensor_readings, node_status));
                            log::debug!("Subscribed client ID {id:}");
                        }
                        ClientApi::Unsubscribe{ id } => {
                           if self.subscribers.remove(&id).is_none(){
                                log::warn!("Removing non-existent subsriber ID");
                            } else {
                                log::debug!("Unsubscribed client ID {id:}");
                            }

                        }
                    }
                }
                // todo tick timeout?
            };
        }
    }

    async fn handle_sensor_stream_task(&mut self, rcv: UnboundedReceiver<NodeEvent>) {
        let data_queue = self.data_queue.clone();
        let node_state = self.sender.clone();
        tokio::spawn(async move {
            tokio::spawn(async move {
                Self::sensor_stream_process(
                    UnboundedReceiverStream::new(rcv),
                    data_queue,
                    node_state,
                )
                .await
            });
        });
    }

    async fn sensor_stream_process(
        mut stream: UnboundedReceiverStream<NodeEvent>,
        sender: UnboundedSender<NodeSensorReading>,
        node_state: UnboundedSender<BrokerEvent>,
    ) {
        log::trace!("Processing NodeEvent receiver as a stream");
        while let Some(msg) = stream.next().await {
            let sender_clone = sender.clone();
            let node_state_clone = node_state.clone();
            match msg {
                NodeEvent::SensorReading(node) => {
                    log::trace!(
                        "Node event: sensor reading! from {:?} data {:?}",
                        node.addr,
                        node.data
                    );

                    if let Err(e) = sender_clone.send(node) {
                        log::error!("Error sending to app {e:}");
                    }
                }
                NodeEvent::SetupError => {
                    log::warn!("Setup error, closing receiver stream");
                    break;
                }
                NodeEvent::SocketError(addr) => {
                    log::warn!(
                        "Socket error on addr {:?}, \
                        closing receiver stream",
                        addr
                    );
                    if let Err(e) = node_state_clone.send(BrokerEvent::NodeTermination((
                        addr,
                        ErrorState::SocketError,
                    ))) {
                        log::error!("Error sending to app {e:}");
                    }
                }
                NodeEvent::NodeTimeout(addr) => {
                    log::warn!(
                        "Node timeout for addr {:?}, \
                        closing receiver stream",
                        addr
                    );
                    if let Err(e) = node_state_clone
                        .send(BrokerEvent::NodeTermination((addr, ErrorState::Timeout)))
                    {
                        log::error!("Error sending to app {e:}");
                    }
                }
            }
        }
        log::warn!("Stream processing func closing");
    }
}

impl Actor for BrokerHandle {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "ClientSubscribeResponse")]
pub struct ClientSubscribe {
    pub id: ClientId,
    pub sensor_readings: UnboundedSender<NodeSensorReading>,
    pub node_status: UnboundedSender<NodeStatus>,
}
type ClientSubscribeResponse = Result<(), BrokerError>;

impl Handler<ClientSubscribe> for BrokerHandle {
    type Result = ClientSubscribeResponse;

    fn handle(&mut self, msg: ClientSubscribe, _ctx: &mut Self::Context) -> Self::Result {
        self.0
            .send(ClientApi::Subscribe {
                id: msg.id,
                sensor_readings: msg.sensor_readings,
                node_status: msg.node_status,
            })
            .map_err(|e| {
                log::error!("Error sending sub to actor {e:}");
                BrokerError::ActorError
            })?;
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "ClientUnsubscribeResponse")]
pub struct ClientUnsubscribe {
    id: ClientId,
}

type ClientUnsubscribeResponse = Result<(), BrokerError>;

impl Handler<ClientUnsubscribe> for BrokerHandle {
    type Result = ClientUnsubscribeResponse;

    fn handle(&mut self, msg: ClientUnsubscribe, _ctx: &mut Self::Context) -> Self::Result {
        self.0
            .send(ClientApi::Unsubscribe { id: msg.id })
            .map_err(|e| {
                log::error!("Error sending unsub to actor {e:}");
                BrokerError::ActorError
            })?;
        Ok(())
    }
}
