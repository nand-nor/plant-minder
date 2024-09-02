//! TUI Front end for rendering pretty graphs and
//! displaying (on a dedicated LCD) when my plants
//! need to be watered, as part of the plant-minder
//! system

pub mod event;
pub mod minder;
pub mod ui;

use pmind_broker::BrokerError;
use pmindb::DatabaseError;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlantMinderError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Broker Error")]
    BrokerError(#[from] BrokerError),
    #[error("Event Handling Error")]
    EventError,
    #[error("Database Error")]
    DatabaseError(#[from] DatabaseError),
}
