//! TUI Front end for rendering pretty graphs and
//! displaying (on a dedicated LCD) when my plants
//! need to be watered, as part of the plant-minder
//! system

pub mod event;
pub mod minder;
pub mod ui;

use pmindb::BrokerCoordinatorError;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlantMinderError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Broker Error")]
    BrokerError(#[from] BrokerCoordinatorError),
    #[error("Event Handling Error")]
    EventError,
}
