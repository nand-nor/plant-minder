#![allow(unused)]
use std::net::{Ipv6Addr, SocketAddrV6};

use chrono::{DateTime, NaiveDateTime, Utc};
use futures::{FutureExt, StreamExt};
use pmindp_sensor::SensorReading;

use thiserror::Error;

use actix::prelude::*;

use pmind_broker::{Eui, NodeEvent, NodeSensorReading, NodeStatus};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

use diesel::QueryDsl;
use diesel::SelectableHelper;
use diesel::{
    insert_into, result::Error::NotFound, Connection, ExpressionMethods, QueryResult, RunQueryDsl,
    SqliteConnection,
};

use crate::{
    models::{MoistureData, NewGasData, NewLightData, NewMoistureData, NewPlant, PlantRecord},
    schema::{gas_data, light_data, moisture_data, plants},
};

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Sqlite Error")]
    SqliteError(#[from] diesel::result::Error),
    #[error("Error, Other")]
    OtherError(#[from] Box<dyn std::error::Error + std::marker::Send + Sync + 'static>),
    #[error("Startup error {0}")]
    StartUpError(String),
}

use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
pub const MIGRATION: EmbeddedMigrations = embed_migrations!("./migrations");

struct PlantDatabase {
    path: std::path::PathBuf,
    conn: SqliteConnection,
}

impl PlantDatabase {
    async fn new(path: &str) -> Result<Self, DatabaseError> {
        let mut conn = SqliteConnection::establish(path)
            .map_err(|_e| DatabaseError::StartUpError(format!("Error connecting to {}", path)))?;

        conn.run_pending_migrations(MIGRATION)?;

        Ok(Self {
            path: path.into(),
            conn,
        })
    }

    fn insert_reading(&mut self, reading: NodeSensorReading) -> Result<(), DatabaseError> {
        let plant: PlantRecord = plants::dsl::plants
            .filter(plants::dsl::addr.eq(&reading.addr.ip().to_string()))
            .first::<PlantRecord>(&mut self.conn)
            .map_err(|e| {
                log::error!(
                    "Error querying for record: {e:} {:?}",
                    reading.addr.ip().to_string()
                );
                e
            })?;

        if let Some(gas) = reading.data.gas {
            let result = insert_into(gas_data::dsl::gas_data)
                .values(NewGasData {
                    parent_plant_eui: plant.eui(),
                    temp: gas.temp,
                    gas: gas.gas as f32,
                    pressure: gas.p,
                    humidity: gas.h,
                    ts: DateTime::from_timestamp(reading.data.ts, 0)
                        .unwrap_or_default()
                        .naive_utc(),
                })
                .returning(crate::models::GasData::as_returning())
                .get_result(&mut self.conn)
                .map_err(|e| {
                    log::error!("Error inserting light data record :( {e:}");
                    e
                })?;
            log::trace!("Gas data insert result {:?}", result);
        }

        if let Some(light) = reading.data.light {
            let result = insert_into(light_data::dsl::light_data)
                .values(NewLightData {
                    parent_plant_eui: plant.eui(),
                    fs: light.fs as f32,
                    lux: light.lux,
                    ts: DateTime::from_timestamp(reading.data.ts, 0)
                        .unwrap_or_default()
                        .naive_utc(),
                })
                .returning(crate::models::LightData::as_returning())
                .get_result(&mut self.conn)
                .map_err(|e| {
                    log::error!("Error inserting light data record :( {e:}");
                    e
                })?;
            log::trace!("Light data insert result {:?}", result);
        }

        let result = insert_into(moisture_data::dsl::moisture_data)
            .values(NewMoistureData {
                parent_plant_eui: plant.eui(),
                moisture: reading.data.soil.moisture as f32,
                temp: reading.data.soil.temp,
                ts: DateTime::from_timestamp(reading.data.ts, 0)
                    .unwrap_or_default()
                    .naive_utc(),
            })
            .returning(crate::models::MoistureData::as_returning())
            .get_result(&mut self.conn)
            .map_err(|e| {
                log::error!("Error inserting moisture data record :( {e:}");
                e
            })?;

        log::trace!("moisture data insert result {:?}", result);

        Ok(())
    }

    fn create_or_modify_plant_record(&mut self, plant: NewPlant) -> Result<(), DatabaseError> {
        match plants::dsl::plants
            .find(&plant.eui)
            .first::<PlantRecord>(&mut self.conn)
        {
            Ok(node) => {
                let addr = plant.addr.clone();
                let name = plant.name.clone();
                let result = insert_into(plants::dsl::plants)
                    .values(plant)
                    .on_conflict(plants::dsl::eui)
                    .do_update()
                    .set((
                        plants::dsl::name.eq(name),
                        plants::dsl::addr.eq(addr),
                        plants::dsl::update_count.eq(node.update_count().wrapping_add(1)),
                    ))
                    .returning(crate::models::PlantRecord::as_returning())
                    .get_result(&mut self.conn)
                    .map_err(|e| {
                        log::error!("Error inserting plant record :( {e:}");
                        e
                    })?;
                log::trace!("Plant record update new: {:?} vs old: {:?}", result, node);
            }
            Err(NotFound) => {
                log::info!(
                    "No plant record in node addr table exists for eui {:?}, inserting",
                    plant.addr
                );
                // Insert the NodeAddr record
                let result = insert_into(plants::dsl::plants)
                    .values(plant)
                    .returning(crate::models::PlantRecord::as_returning())
                    .get_result(&mut self.conn)
                    .map_err(|e| {
                        log::error!("Error inserting node addr record :( {e:}");
                        e
                    })?;

                log::trace!("Plant record insert result: {:?}", result);
            }
            Err(e) => {
                log::error!("Error checking :( {e:}");
                return Err(DatabaseError::from(e));
            }
        };
        Ok(())
    }
}

impl Actor for PlantDatabase {
    type Context = Context<Self>;
}

#[derive(Debug, Message)]
#[rtype(result = "NodeSensorDataResponse")]
pub struct NodeSensorData(NodeSensorReading);

type NodeSensorDataResponse = Result<(), DatabaseError>;

impl Handler<NodeSensorData> for PlantDatabase {
    type Result = NodeSensorDataResponse;

    fn handle(&mut self, msg: NodeSensorData, _ctx: &mut Self::Context) -> Self::Result {
        log::trace!("database actor NodeSensorData called, msg: {msg:?}");
        self.insert_reading(msg.0)?;

        Ok(())
    }
}

#[derive(Debug, Message)]
#[rtype(result = "CreateOrModifyResponse")]
pub struct CreateOrModify {
    pub eui: Eui,
    pub addr: Ipv6Addr,
    pub name: String,
}

type CreateOrModifyResponse = Result<(), DatabaseError>;

impl Handler<CreateOrModify> for PlantDatabase {
    type Result = CreateOrModifyResponse;

    fn handle(&mut self, msg: CreateOrModify, _ctx: &mut Self::Context) -> Self::Result {
        log::trace!("database actor CreateOrModify called, msg: {msg:?}");
        let record = NewPlant::new(msg);
        self.create_or_modify_plant_record(record)?;
        Ok(())
    }
}

pub struct SubscriptionHandler {
    db_registry_conn_handle: Option<tokio::task::JoinHandle<Result<(), DatabaseError>>>,
    db_sensor_stream_conn_handle: Option<tokio::task::JoinHandle<Result<(), DatabaseError>>>,
}

impl SubscriptionHandler {
    fn new() -> Self {
        Self {
            db_registry_conn_handle: None,
            db_sensor_stream_conn_handle: None,
        }
    }

    async fn spawn_db_conn_registry_task(
        &mut self,
        db: Addr<PlantDatabase>,
        mut status_rcvr: UnboundedReceiver<NodeStatus>,
    ) {
        let db = db.clone();
        let handle = tokio::spawn(async move {
            while let Some(event) = status_rcvr.recv().await {
                match event {
                    NodeStatus::Registration(reg) => {
                        if let Err(e) = db
                            .send(CreateOrModify {
                                eui: reg.0,
                                addr: reg.1,
                                name: reg.2,
                            })
                            .await
                        {
                            log::error!("database actor handle error {e:}");
                        }
                    }
                    NodeStatus::Termination((_addr, _error_state)) => {
                        // TODO: maybe have timer that starts, if node does not come
                        // back within a week or two, evict?
                        log::info!("TODO! report this to DB, node terminated");
                    }
                }
            }

            log::warn!("DB node registry task exiting");
            Ok(())
        });

        self.db_registry_conn_handle = Some(handle);
    }

    pub async fn exec_task_loops(&mut self) {
        self.db_registry_conn_handle.take().unwrap().await.ok();
        self.db_sensor_stream_conn_handle.take().unwrap().await.ok();
    }

    async fn spawn_db_conn_sensor_stream_task(
        &mut self,
        db: Addr<PlantDatabase>,
        mut receiver: UnboundedReceiver<NodeSensorReading>,
    ) {
        let db = db.clone();
        let handle = tokio::spawn(async move {
            loop {
                while let Some(data) = receiver.recv().await {
                    if let Err(e) = db.send(NodeSensorData(data)).await {
                        log::error!("Error sending to db handle {e:}");
                    }
                }
            }
        });
        self.db_sensor_stream_conn_handle = Some(handle);
    }
}

impl Drop for SubscriptionHandler {
    fn drop(&mut self) {
        if let Some(events) = &self.db_registry_conn_handle {
            events.abort();
        }

        if let Some(streams) = &self.db_sensor_stream_conn_handle {
            streams.abort();
        }
    }
}

pub struct PlantDatabaseHandler {
    db: Addr<PlantDatabase>,
}

impl PlantDatabaseHandler {
    async fn new(path: &str) -> Result<Self, DatabaseError> {
        Ok(Self {
            db: PlantDatabase::new(path).await?.start(),
        })
    }

    pub async fn new_with_db_conn_tasks(
        path: &str,
    ) -> Result<
        (
            Self,
            UnboundedSender<NodeSensorReading>,
            UnboundedSender<NodeStatus>,
        ),
        DatabaseError,
    > {
        let (stream_tx, stream_rx) = unbounded_channel();
        let (registration_tx, registration_rx) = unbounded_channel();

        let handler = PlantDatabaseHandler::new(path).await?;

        let mut sub_handler = SubscriptionHandler::new();

        sub_handler
            .spawn_db_conn_registry_task(handler.db.clone(), registration_rx)
            .await;
        sub_handler
            .spawn_db_conn_sensor_stream_task(handler.db.clone(), stream_rx)
            .await;

        tokio::spawn(async move {
            sub_handler.exec_task_loops().await;
            log::warn!("database handle exiting event loop");
        });

        Ok((handler, stream_tx, registration_tx))
    }
}
