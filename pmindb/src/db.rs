#![allow(unused)]
use std::net::Ipv6Addr;

use pmindp_sensor::SensorReading;
use rusqlite::{params, Connection, Error as SqliteErr, Result};
use thiserror::Error;

use actix::prelude::*;

use crate::Eui;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Sqlite Error")]
    SqliteError(#[from] SqliteErr),
}

struct Plant {
    /// Each sensor will retaina  unique EUI that
    /// persists across resets. Use this value to
    /// retain associations between sensors and
    /// the plants they are tracking
    sensor_id: Eui,
    species: String,
    //conditions
}

struct PlantDatabase {
    path: std::path::PathBuf,
    conn: Connection,
    // tables: Vec<PlantTable>,
}

pub(crate) struct PlantDatabaseHandler {
    db: PlantDatabase,
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
            params![plant.sensor_id, plant.species],
        )?;

        Ok(())
    }
}

impl PlantDatabaseHandler {
    pub fn new(path: std::path::PathBuf) -> Result<Self, DatabaseError> {
        Ok(Self {
            db: PlantDatabase::new(path)?,
        })
    }
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
