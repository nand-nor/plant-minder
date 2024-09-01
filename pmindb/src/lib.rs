//! The `pmindb` crate defines the database functionality for the plant-minder
//! system running on the RPI. Maintains database via
//! [`db::PlantDatabaseHandler`]
//! an [`actix::Actor`] oject, to do the following:
//!    1. Record / track node info, using  [`pmind_broker::Eui`]'s to associate nodes
//!       & their current IPv6 address with a plant record. Each plant record
//!       has an associated historical record of sensor data
//!    2. Record / track sensor data, where the monitor/node layer provides
//!       streams of sensor data for a given plant in the database
//!    3. IPv6 addresses allow received sensor data to be associated to an
//!       [`pmind_broker::Eui`] which ties back to the original plant record, even if
//!       the address changes

mod db;
mod models;
mod schema;

use chrono::NaiveDateTime;
pub use db::{DatabaseError, PlantDatabaseHandler};
use pmind_broker::{Eui, NodeSensorReading};

#[async_trait::async_trait]
pub trait PlantMinderDatabase {
    /// Get all available sensor data for the [`pmind_broker::Eui`] since
    /// the provided timestamp. A timestamp of [`chrono::Local::now().naive_dt()`]
    /// will return an empty vector
    async fn get_full_history_since_ts(
        &self,
        eui: Eui,
        timestamp: NaiveDateTime,
    ) -> Result<Vec<NodeSensorReading>, DatabaseError>;

    /// Get all available sensor data for the [`pmind_broker::Eui`] 
    /// in the stored database
    async fn get_full_history(&self, eui: Eui) -> Result<Vec<NodeSensorReading>, DatabaseError>;
    
}

#[async_trait::async_trait]
impl PlantMinderDatabase for PlantDatabaseHandler {
    async fn get_full_history(&self, eui: Eui) -> Result<Vec<NodeSensorReading>, DatabaseError> {
        todo!()
    }

    async fn get_full_history_since_ts(
        &self,
        eui: Eui,
        timestamp: NaiveDateTime,
    ) -> Result<Vec<NodeSensorReading>, DatabaseError> {
        todo!()
    }
}
