//! The `pmindb` crate defines the broker layer for the plant-minder
//! system running on the RPI. This layer is composed of two distinct
//! [`actix::Actor`] ojects that either interface with a database or
//! interface with the openthread stack (via the otbr-agent).
//!
//! The responsibilities of the broker layer are the following:
//! 1. Interface with the otbr-agent layer to monitor thread mesh
//!    for new / reset sensor nodes via [`cli::OtMonitor`](cli/struct.OtMonitor.html),
//!    an [`actix::Actor`] oject
//! 2. For each active node on the mesh, this actor does the following:
//!    a. Registers and maintains active CoAP (observer client) status, by
//!       spawning a task to manage a socket that receives sensor data, in a
//!       1:1 mapping where each active node gets it's own port
//!    c. Using [`tokio_stream::wrappers::UnboundedReceiverStream`] objects,
//!       stream sensor data to the database tracking sensors/plants/readings
//!    c. Tracks available ports to use as new nodes come online or existing
//!       nodes have a reset event, frees up ports when not in use, and
//!       generally tracks when nodes fall off the network & log appropriately
//! 3. Maintain database via [`db::PlantDatabaseHandler`](db/struct.PlantDatabaseHandler.html)
//!    an [`actix::Actor`] oject
//!    a. Record / track node info, using  [`crate::Eui`]'s to associate nodes
//!       & their current IPv6 address with a plant record. Each plant record
//!       has an associated historical record of sensor data
//!    b. Record / track sensor data, where the monitor/node layer provides
//!       streams of sensor data for a given plant in the database
//!    b. IPv6 addresses allow received sensor data to be associated to an
//!       [`crate::Eui`] which ties back to the original plant record, even if
//!       the address changes

mod broker;
mod db;
mod monitor;
mod node;

mod client;

pub use broker::{BrokerCoordinator, BrokerCoordinatorError};
pub(crate) use client::{OtCliClient, OtClient, OtClientError};
pub(crate) use db::PlantDatabaseHandler;
pub(crate) use monitor::{OtMonitor, OtMonitorError};
pub use node::{NodeEvent, NodeSensorReading, Registration};

/// Extended Unique Identifier: each node should have a unique EUI that
/// is always the same even across node cpu resets / power events
pub type Eui = [u8; 6];

/// Routing locator
pub type Rloc = u16;
