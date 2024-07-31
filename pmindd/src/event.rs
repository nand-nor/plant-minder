use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyModifiers};
use futures::{FutureExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;

use pmindb::{NodeEvent, NodeSensorReading, Registration};

use crate::{
    minder::{PlantMinder, PlantMinderResult, NUM_TABS},
    PlantMinderError,
};

/// Backend event
pub enum Event {
    Tick,
    AppCmd(AppCmd),
    NodeRegistration(Registration),
    SensorNodeEvent(UnboundedReceiver<NodeEvent>),
}

/// App command is derived
/// from user input
#[derive(Debug)]
pub enum AppCmd {
    Quit,
    Invalid,
    Error,
    Next,
    Back,
    Up,
    Down,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
    handler: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(
        tick_rate: u64,
        node_data_rx: UnboundedReceiver<UnboundedReceiver<NodeEvent>>,
        node_reg_rx: UnboundedReceiver<Registration>,
    ) -> Self {
        let tick_rate = Duration::from_secs(tick_rate);
        let (sender, receiver) = mpsc::unbounded_channel();
        let _sender = sender.clone();

        let mut node_event_stream = UnboundedReceiverStream::new(node_data_rx);
        let mut node_reg_stream = UnboundedReceiverStream::new(node_reg_rx);

        let handler = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick = tokio::time::interval(tick_rate);

            loop {
                let tick_delay = tick.tick();
                let crossterm_event = reader.next().fuse();
                let node_event_stream = node_event_stream.next().fuse();
                let node_reg_stream = node_reg_stream.next().fuse();

                tokio::select! {
                  _ = _sender.closed() => {
                    break;
                  }
                  _ = tick_delay => {
                    _sender.send(Event::Tick).unwrap();
                  }
                  Some(Ok(evt)) = crossterm_event => {
                    match evt {
                      CrosstermEvent::Key(key) => {
                        if key.kind == crossterm::event::KeyEventKind::Press {

                          let cmd = match key.code {
                            KeyCode::Esc | KeyCode::Char('q')  | KeyCode::Char('Q') => {
                                AppCmd::Quit
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                if key.modifiers == KeyModifiers::CONTROL {
                                    AppCmd::Quit
                                } else {
                                    AppCmd::Invalid
                                }
                            },

                            KeyCode::Tab => AppCmd::Next,
                            KeyCode::BackTab => AppCmd::Back,
                            KeyCode::Up => AppCmd::Up,
                            KeyCode::Down => AppCmd::Down,
                            _ => AppCmd::Invalid,
                        };
                        _sender.send(Event::AppCmd(cmd)).unwrap();

                        }
                      },
                      e => {
                        log::warn!("Untracked term event {e:?}");
                      }
                    }
                  }
                 Some(node_evt) = node_event_stream => {
                    log::debug!("node event {node_evt:?}");
                    _sender.send(Event::SensorNodeEvent(node_evt)).unwrap();

                  }
                  Some(reg) = node_reg_stream => {
                    log::debug!("Node registration {reg:?}");
                    _sender.send(Event::NodeRegistration(reg)).unwrap();

                  }

                };
            }
        });
        Self {
            sender,
            receiver,
            handler,
        }
    }

    pub async fn next(&mut self) -> PlantMinderResult<Event> {
        self.receiver
            .recv()
            .await
            .ok_or(PlantMinderError::EventError)
    }
}

pub async fn handle_app_cmd(cmd: AppCmd, app: &mut PlantMinder) {
    match cmd {
        AppCmd::Quit => app.quit(),
        AppCmd::Next => {
            if app.tab == NUM_TABS - 1 {
                app.tab = 0;
            } else {
                app.tab += 1;
            }
        }
        AppCmd::Back => {
            if app.tab == 0 {
                app.tab = NUM_TABS - 1;
            } else {
                app.tab -= 1;
            }
        }
        AppCmd::Down => {
            app.row += 1;
        }
        AppCmd::Up => {
            if app.row == 0 {
                if !app.node_addrs.is_empty() {
                    app.row = app.node_addrs.len() - 1;
                } else {
                    app.row = 0;
                }
            } else {
                app.row -= 1;
            }
        }
        e => {
            log::debug!("Unimplemented event received {e:?}");
            // drop it for now
        }
    }
}

pub async fn handle_sensor_stream_task(app: &mut PlantMinder, rcv: UnboundedReceiver<NodeEvent>) {
    let data_queue = app.data_queue.clone();
    let handle = tokio::spawn(async move {
        tokio::spawn(async move {
            sensor_stream_process(UnboundedReceiverStream::new(rcv), data_queue).await
        });
    });
    app.sensor_streams.push(handle);
}

async fn sensor_stream_process(
    mut stream: UnboundedReceiverStream<NodeEvent>,
    sender: UnboundedSender<NodeSensorReading>,
    // sender: UnboundedSender<NodeSensorReading>,
) {
    log::trace!("Processing NodeEvent receiver as a stream");
    while let Some(msg) = stream.next().await {
        let sender_clone = sender.clone();
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

                if let Err(e) = sender_clone.send(node) {
                    log::error!("Error sending to app {e:}");
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

pub async fn handle_node_reg_task(app: &mut PlantMinder, reg: Registration) {
    app.node_registration(reg).await;
}
