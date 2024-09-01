//! The `pmindb` crate defines the database functionality for the plant-minder
//! system running on the RPI. Maintains database via
//! [`db::PlantDatabaseHandler`](db/struct.PlantDatabaseHandler.html)
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

pub(crate) use db::PlantDatabaseHandler;
