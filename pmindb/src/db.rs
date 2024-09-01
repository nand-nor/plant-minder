#![allow(unused)]
use std::net::Ipv6Addr;

use pmindp_sensor::SensorReading;
use rusqlite::{params, Connection, Error as SqliteErr, Result};
use thiserror::Error;

use actix::prelude::*;

use pmind_broker::{Eui, NodeEvent};
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Sqlite Error")]
    SqliteError(#[from] SqliteErr),
}

struct Plant {
    /// Each sensor will have a unique EUI that
    /// persists across resets. Use this value to
    /// retain associations between sensors and
    /// the plants they are tracking
    sensor_id: Eui,
    /// Each sensor reports it's own plant name
    /// distinct from species. Species is also
    /// self-reported by each sensor and used
    /// to generate the [`SpeciesRecord`]
    plant_display_name: String,
    /// Species Record will record ideal ranges
    /// for soil moister, light, and other
    /// parameters which will inform the displayed
    /// value of when watering is needed
    record: SpeciesRecord,
}

pub struct SpeciesRecord {
    species: String,
    /// Desired min & max moisture,
    /// based on plant species & growth stage
    moisture_range: pmindp_sensor::Range<u16>,
    /// Desired min & max lux,
    /// based on plant species & growth stage
    lux_range: pmindp_sensor::Range<f32>,
    /// Growth stage
    growth_stage: pmindp_sensor::GrowthStage,
}

struct PlantDatabase {
    path: std::path::PathBuf,
    conn: Connection,
    // tables: Vec<PlantTable>,
}

pub(crate) struct PlantDatabaseHandler {
    db: PlantDatabase,
    db_registry_conn_handle: Option<tokio::task::JoinHandle<Result<(), DatabaseError>>>,
    db_sensor_stream_conn_handle: Option<tokio::task::JoinHandle<Result<(), DatabaseError>>>,
}

impl PlantDatabase {
    pub fn new(path: std::path::PathBuf) -> Result<Self, DatabaseError> {
        let conn = Connection::open(path.clone())?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS plants (
                sensor_id INTEGER PRIMARY KEY,
                species TEXT NOT NULL
            )",
            (),
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS readings (
                sensor_id INTEGER PRIMARY KEY,
                moisture INTEGER,
                temperature FLOAT
            )",
            (),
        )?;

        Ok(Self { path, conn })
    }

    pub(crate) fn insert_reading(
        &self,
        reading: SensorReading,
        sensor_id: u16,
    ) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO readings (sensor_id, moisture, temperature) VALUES (?1), (?2), (?3)",
            params![sensor_id, reading.soil.moisture, reading.soil.temp],
        )?;

        Ok(())
    }

    pub(crate) fn add_new_plant(&self, plant: Plant) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO readings (sensor_id, species) VALUES (?1), (?2), (?3)",
            params![plant.sensor_id, plant.record.species],
        )?;

        Ok(())
    }

}

impl PlantDatabaseHandler {
    pub fn new(path: std::path::PathBuf) -> Result<Self, DatabaseError> {
        Ok(Self {
            db: PlantDatabase::new(path)?,
            db_registry_conn_handle: None,
            db_sensor_stream_conn_handle: None,
            
        })
    }

        /*pub async fn new(path: std::path::PathBuf) -> Result<Self, EventRouterError> {
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
            .spawn_child_mon_task(
                Duration::from_secs(25),
                ot_mon_handle,
                stream_tx,
                registration_tx,
            )
            .await;
        broker
            .spawn_db_conn_sensor_stream_task(database_handle, stream_rx)
            .await;

        Ok(broker)
    }*/

     /*async fn spawn_db_conn_registry_task(
        &mut self,
        db: Addr<PlantDatabaseHandler>,
        mut registration_rcvr: UnboundedReceiver<(Eui, Ipv6Addr, String)>,
    ) {
        let handle = tokio::spawn(async move {
            while let Some((eui, rcv, name)) = registration_rcvr.recv().await {
                log::trace!("Node being added to DB {:?} addr {:?}", eui, rcv);
                if let Err(e) = db.send(CreateOrModify { eui, ip: rcv, name }).await {
                    log::error!("database actor handle error {e:}");
                }
            }

            log::warn!("DB node registry task exiting");
            Ok(())
        });

        self.db_registry_conn_handle = Some(handle);
    }*/


     /*pub async fn exec_task_loops(&mut self) {
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
                    log::debug!("Reading! from {:?} data {:?}", node.addr, node.data);

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
    }*/
}

impl Actor for PlantDatabaseHandler {
    type Context = Context<Self>;
}

#[derive(Debug, Message)]
#[rtype(result = "NodeSensorReadingResponse")]
pub struct NodeSensorReading(pub (Ipv6Addr, SensorReading));

type NodeSensorReadingResponse = Result<(), DatabaseError>;

impl Handler<NodeSensorReading> for PlantDatabaseHandler {
    type Result = NodeSensorReadingResponse;

    fn handle(&mut self, msg: NodeSensorReading, _ctx: &mut Self::Context) -> Self::Result {
        // TODO associate Ipv6Addr in reading with Eui to get Plant entry
        // then associate sensor readying with said plant entry

        log::info!("Got a sensor reading :) {:?}", msg);
        Ok(())
    }
}

#[derive(Debug, Message)]
#[rtype(result = "CreateOrModifyResponse")]
pub struct CreateOrModify {
    pub eui: Eui,
    pub ip: Ipv6Addr,
    pub name: String,
}

type CreateOrModifyResponse = Result<(), DatabaseError>;

impl Handler<CreateOrModify> for PlantDatabaseHandler {
    type Result = CreateOrModifyResponse;

    fn handle(&mut self, msg: CreateOrModify, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("Got a new node reg :) {:?}", msg);

        // TODO maintain list of EUI + currently associated Ipv6Addr
        // at any point the node may reset itself / go offline so need to
        // determine if the EUI is already in the db, in which case it is a modify op
        // where the new ip addr should replace the old one, otherwise it is a
        // create op

        Ok(())
    }
}

async fn spawn_db_conn_sensor_stream_task(
    db: Addr<PlantDatabaseHandler>,
    mut receiver: UnboundedReceiver<UnboundedReceiver<NodeEvent>>,
) -> JoinHandle<()> {
    let handle = tokio::spawn(async move {
        loop {
            while let Some(rcv) = receiver.recv().await {
                let db_clone = db.clone();
                tokio::spawn(async move {
                    process(UnboundedReceiverStream::new(rcv), db_clone).await
                });
            }
        }
    });
    handle
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
                log::debug!("Reading! from {:?} data {:?}", node.addr, node.data);

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



impl Drop for PlantDatabaseHandler {
    fn drop(&mut self) {
        if let Some(events) = &self.db_registry_conn_handle {
            events.abort();
        }

        if let Some(streams) = &self.db_sensor_stream_conn_handle {
            streams.abort();
        }
    }
}