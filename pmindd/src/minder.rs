use pmindb::{Eui, NodeSensorReading, Registration};
use ratatui::widgets::TableState;
use std::{collections::HashMap, net::Ipv6Addr};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::PlantMinderError;

pub type PlantMinderResult<T> = std::result::Result<T, PlantMinderError>;

#[derive(Debug)]
pub struct PlantMinder {
    pub running: bool,
    pub sensor_streams: Vec<tokio::task::JoinHandle<()>>,
    pub state: TableState,
    pub data_queue: UnboundedSender<NodeSensorReading>,
    pub data_queue_rx: UnboundedReceiver<NodeSensorReading>,
    pub node_data: HashMap<Ipv6Addr, Vec<NodeSensorReading>>,
    pub node_addrs: HashMap<Eui, Ipv6Addr>,
    read_timeout: u64,
}

pub struct Nodes {}

impl PlantMinder {
    pub fn new(read_timeout: u64) -> Self {
        let (data_queue, data_queue_rx) = unbounded_channel();

        Self {
            running: true,
            sensor_streams: vec![],
            state: TableState::default().with_selected(0),
            data_queue,
            data_queue_rx,
            node_data: HashMap::new(),
            node_addrs: HashMap::new(),
            read_timeout,
        }
    }

    pub async fn tick(&mut self) {
        let mut buffer: Vec<NodeSensorReading> = vec![];
        self.recv_many(&mut buffer, 10).await;
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn recv_many(&mut self, buffer: &mut Vec<NodeSensorReading>, limit: usize) {
        let size = tokio::select! {
            size = self.data_queue_rx.recv_many(buffer, limit) => {
                size
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.read_timeout)) => {
                0
            }
        };

        if size != 0 {
            let keys = self
                .node_data
                .clone()
                .into_keys()
                .collect::<Vec<Ipv6Addr>>();

            keys.iter().for_each(|&key| {
                let new_data = buffer
                    .iter()
                    .filter_map(|&e| if *e.addr.ip() == key { Some(e) } else { None })
                    .collect::<Vec<NodeSensorReading>>();

                if !new_data.is_empty() {
                    self.new_data(key, new_data);
                }
            });
        }
    }

    pub async fn node_registration(&mut self, reg: Registration) {
        let history = {
            if let Some(previous) = self.node_addrs.get(&reg.0) {
                if let Some(entry) = self.node_data.get(previous) {
                    entry.clone()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        };

        self.node_data
            .entry(reg.1)
            .and_modify(|a| a.clone_from(&history))
            .or_insert(history);
    }

    fn new_data(&mut self, key: Ipv6Addr, data: Vec<NodeSensorReading>) {
        self.node_data
            .entry(key)
            .and_modify(|a| a.extend_from_slice(&data))
            .or_default();
    }
}
