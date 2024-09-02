use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyModifiers};
use futures::{FutureExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;

use pmind_broker::{NodeSensorReading, NodeStatus};

use crate::{minder::PlantMinderResult, PlantMinderError};

/// Backend event
pub enum Event {
    Tick,
    AppCmd(AppCmd),
    NodeState(NodeStatus),
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
        node_data_rx: UnboundedReceiver<NodeSensorReading>,
        node_state_rx: UnboundedReceiver<NodeStatus>,
        client_data_tx: UnboundedSender<NodeSensorReading>,
    ) -> Self {
        let tick_rate = Duration::from_secs(tick_rate);
        let (sender, receiver) = mpsc::unbounded_channel();
        let _sender = sender.clone();

        let mut node_data_stream = UnboundedReceiverStream::new(node_data_rx);
        let mut node_state_stream = UnboundedReceiverStream::new(node_state_rx);

        let handler = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick = tokio::time::interval(tick_rate);

            loop {
                let tick_delay = tick.tick();
                let crossterm_event = reader.next().fuse();

                let node_data_stream = node_data_stream.next().fuse();
                let node_state_stream = node_state_stream.next().fuse();

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
                 Some(state) = node_state_stream => {
                    log::debug!("Node state event {state:?}");
                    _sender.send(Event::NodeState(state)).unwrap();
                  }
                  Some(data) = node_data_stream => {
                    log::debug!("Node data event {data:?}");
                    client_data_tx.send(data).unwrap();
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
