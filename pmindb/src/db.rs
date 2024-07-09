use pmindp_sensor::{ATSAMD10SensorReading, SensorReading};
use rusqlite::{params, Connection, Error as SqliteErr, Result};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Sqlite Error")]
    SqliteError(#[from] SqliteErr),
}

pub(crate) struct Plant {
    sensor_id: u16, // node rloc
    species: String,
    //conditions
}

pub(crate) struct PlantDatabase {
    path: std::path::PathBuf,
    conn: Connection,
    // tables: Vec<PlantTable>,
}

/*
struct PlantTable {
    name: String,
   // date_created: <>
  //  last_updated: <>
  // size?
    schema: String
}*/

impl PlantDatabase {
    pub fn new(path: std::path::PathBuf) -> Result<Self, DatabaseError> {
        let conn = Connection::open(path.clone())?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS plants (
                sensor_id INTEGER PRIMARY KEY,
                species TEXT NOT NULL
            )",
            (), // empty list of parameters.
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS readings (
                sensor_id INTEGER PRIMARY KEY,
                moisture INTEGER,
                temperature FLOAT
            )",
            (), // empty list of parameters.
        )?;

        Ok(Self { path, conn })
    }

    //pub(crate) fn add_new_reading(reading: dyn SensorReading, sensor_id: u16) -> Result<(), DatabaseError>{

    pub(crate) fn insert_reading(
        &self,
        reading: ATSAMD10SensorReading,
        sensor_id: u16,
    ) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO readings (sensor_id, moisture, temperature) VALUES (?1), (?2), (?3)",
            params![sensor_id, reading.moisture, reading.temperature],
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
