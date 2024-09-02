//! The `pmind-broker` crate defines the broker layer for the plant-minder
//! system, which runs on the RPI. This layer is composed of [`actix::Actor`]
//! ojects that handle interfacing with subscribing client(s), the
//! monitoring/managment of server nodes (serving sensor data), and the
//! routing of served sensor data (so that it is available to subscribing
//! clients) as well as other events related to nodes on the mesh.
//!
//! The crate defines a top-level [`Broker`] object, which exposes an actor
//! handle as well as manages a number of internal actors, that act in coordination
//! to meet the following responsibilities:
//! 1. Interface with the openthread stack (via the otbr-agent) to monitor
//!    thread mesh for new / reset sensor nodes via [`OtMonitor`],
//!    an [`actix::Actor`] oject. For each active node on the mesh, this
//!    actor does the following:
//!    a. Register and maintain active CoAP subscription (as an observer client)
//!       to request nodes to start serving sensor data.
//!    b. The actor spawns a dedicated task for each node to open and manage a
//!       socket to receive sensor data, in a 1:1 mapping where each active node
//!       gets it's own port.
//!    b. The actor also tracks available ports to use as new nodes come online or
//!       existing nodes have a reset event, freeing up ports when not in use/when
//!       a node resets, and generally tracks when nodes fall off the network & logs
//!       appropriately / generates an event
//! 2. Route received sensor data and node events so that it is available to any
//!    subscribing clients. The [`EventRouter`] actor performs the set up and
//!    coordination between the , including the [`OtMonitor`] object, to enable this.
//!    a. Using [`tokio_stream::wrappers::UnboundedReceiverStream`] objects, the dedicated
//!       task set up for each node streams sensor data to a queue that is then available
//!       for subscribing clients to consume
//!
//! The [`Broker`] object exposes a client subscription API to enable subscribers to
//! receive and process sensor data as it is received from each discovered node on
//! the mesh. See the below example
//!
//! # Examples
//! ```rust
//! #[actix::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!    let broker_handle = pmind_broker::broker(tokio::time::Duration::from_secs(15), 500)
//!         .await
//!         .map_err(|e| {
//!             log::error!("Error creating broker & handle {e:}");
//!             e
//!         })?;
//!
//!     let (sensor_stream_tx, sensor_stream_rx) = tokio::sync::mpsc::unbounded_channel();
//!     let (node_state_tx, node_state_rx) = tokio::sync::mpsc::unbounded_channel();
//!
//!     // The provided client ID must be unique for each subscriber
//!     broker_handle
//!         .send(pmind_broker::ClientSubscribe {
//!             id: 0,
//!             sensor_readings: sensor_stream_tx,
//!             node_status: node_state_tx,
//!         })
//!         .await
//!         .map_err(|e| {
//!            log::error!("Error sending client subscribe request {e:}");
//!             e
//!         })??;
//! ```

mod broker;
mod client;
mod monitor;
mod node;
mod router;

pub(crate) use client::{OtCliClient, OtClient, OtClientError};
pub(crate) use monitor::{OtMonitor, OtMonitorError};
pub(crate) use router::{EventRouter, EventRouterError};

pub use broker::{broker, Broker, BrokerError, ClientSubscribe, ClientUnsubscribe};
pub use node::{ErrorState, NodeEvent, NodeSensorReading, NodeState, NodeStatus};

/// [`Eui`] is the Extended Unique Identifier: each node should have a
/// unique EUI that persists across node cpu resets / power events
pub type Eui = [u8; 6];

/// Routing locator
pub type Rloc = u16;

/// [`ClientId`] is used with subscribing to broker events
pub type ClientId = u32;

/// Node [`Registration`] information for client subscribers
pub type Registration = (Eui, std::net::Ipv6Addr, String);

// Used to limit rendered plant names
const MAX_PLANT_NAME_SIZE: usize = 20;

// Define the number of seconds before a node is considered "Timed out"
// a.k.a. dropped off the network
const DEFAULT_TIMEOUT: u64 = 100;
