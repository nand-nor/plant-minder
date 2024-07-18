use crate::{Eui, OtClient, OtClientError, Rloc};
use actix::prelude::*;

use std::{
    collections::{HashMap, HashSet},
    net::Ipv6Addr,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OtMonitorError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("OT Client Error")]
    OtClientError(#[from] OtClientError),
    #[error("Port Error {0}")]
    PortError(String),
}

pub type NodeRcvPort = u16;
pub struct Ports {
    base: NodeRcvPort,
    ports: HashSet<NodeRcvPort>,
}

impl Ports {
    pub fn new(base: NodeRcvPort, size: u16) -> Result<Self, OtMonitorError> {
        if base.checked_add(size).is_none() {
            let msg = "Incompatible port base + size requested".to_string();
            log::error!("{}", msg);
            return Err(OtMonitorError::PortError(msg));
        }

        let mut ports = HashSet::new();
        for i in 0..size as usize {
            ports.insert(base + i as u16);
        }

        Ok(Self { base, ports })
    }

    /// Returns port to the set to indicate free to use
    pub fn mark_port_free_to_use(&mut self, port: NodeRcvPort) {
        if !self.ports.insert(port) {
            log::error!("Port was already in free port set");
        }
    }

    /// Removes port from the set to indicate it is in use. If registration
    /// fails, port must be returned to the set, to mark it as free to use
    pub fn get_free_port(&mut self) -> Result<NodeRcvPort, OtMonitorError> {
        if self.ports.is_empty() {
            let msg = "No free ports available".to_string();
            log::error!("{}", msg);
            return Err(OtMonitorError::PortError(msg));
        }

        let mut port = self.base;

        // Grab the top free port in set
        if let Some(top) = self.ports.iter().next() {
            port = *top;
        }

        // remove it
        self.ports.take(&port);

        Ok(port)
    }
}

pub struct OtMonitor {
    /// Track nodes that are currently registered
    nodes: HashMap<NodeRcvPort, Registration>,
    /// Pool of free ports to grab from
    ports: Ports,
    addr: Ipv6Addr,
    /// Dynamic trait object that implements the needed traits to
    /// interface with the otbr-agent layer
    ot_client: Box<dyn OtClient>,
}

impl OtMonitor {
    pub fn new(ot_client: Box<dyn OtClient>) -> Self {
        let addr = {
            if let Ok(addr) = ot_client.get_omr_ip() {
                addr
            } else {
                // This will trigger logic to update later if possible
                Ipv6Addr::from([0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1])
            }
        };

        // 100 ports should be more than enough
        let ports = Ports::new(1213, 100).unwrap();

        Self {
            nodes: HashMap::default(),
            addr,
            ot_client,
            ports,
        }
    }

    pub fn get_omr_ip(&self) -> Result<Ipv6Addr, OtMonitorError> {
        self.ot_client.get_omr_ip().map_err(|e| {
            log::error!("Ot Client unable to get OMR ip {e:}");
            OtMonitorError::from(e)
        })
    }

    pub fn check_addr_update_needed(&mut self) -> Result<bool, OtMonitorError> {
        Ok(self.addr.is_loopback()
            || self.addr.segments()[0] != self.ot_client.get_omr_prefix()?.addr().segments()[0])
    }

    pub fn update_addr(&mut self) -> Result<(), OtMonitorError> {
        self.addr = self.get_omr_ip()?;
        Ok(())
    }

    pub fn get_nodes(&self) -> Result<Vec<(Rloc, Ipv6Addr)>, OtMonitorError> {
        Ok(self
            .ot_client
            .get_child_ips()?
            .iter()
            .filter_map(|(rloc, ip)| {
                if ip.segments()[0] == self.addr.segments()[0] {
                    // Only push addrs that match the addr scope (OMR)
                    Some((rloc, ip))
                } else {
                    None
                }
            })
            .map(|(r, p)| (*r, *p))
            .collect())
    }

    pub fn register_node(&mut self, node: Registration) -> Result<(), OtMonitorError> {
        log::debug!("Registering node rloc {} : port {}", node.rloc, node.port);
        let port = node.port;

        self.nodes
            .entry(port)
            .and_modify(|a| *a = node.clone())
            .or_insert(node);

        Ok(())
    }

    pub fn evict_node(&mut self, key: &NodeRcvPort) {
        self.nodes.remove(key);
        self.ports.mark_port_free_to_use(*key);
    }

    pub fn get_free_port(&mut self) -> Result<NodeRcvPort, OtMonitorError> {
        self.ports.get_free_port()
    }

    pub fn return_port(&mut self, port: NodeRcvPort) {
        self.ports.mark_port_free_to_use(port);
    }
}

impl Actor for OtMonitor {
    type Context = Context<Self>;
}

/// Check to see if any previously registered nodes have
/// fallen off the network
#[derive(Message)]
#[rtype(result = "NodeStatusResponse")]
pub struct GetNodeStatus;
type NodeStatusResponse = Result<Vec<(u16, Ipv6Addr)>, OtMonitorError>;

impl Handler<GetNodeStatus> for OtMonitor {
    type Result = NodeStatusResponse;

    fn handle(&mut self, _msg: GetNodeStatus, _ctx: &mut Self::Context) -> Self::Result {
        let active_nodes = self.get_nodes()?;

        // Iterate through the hashmap of currently registered nodes;
        // if any are not in the active node list then
        // they are missing
        let missing_nodes = self
            .nodes
            .iter()
            .filter_map(|(key, node)| {
                if let Some(_found) = active_nodes
                    .iter()
                    .find(|(r, i)| *r == node.rloc && *i == node.ip)
                {
                    None
                } else {
                    Some((key, (node.rloc, node.ip)))
                }
            })
            .map(|(key, (rloc, ip))| (*key, (rloc, ip)))
            .collect::<Vec<_>>();

        // clean up internal info based on results
        for (key, _) in &missing_nodes {
            self.evict_node(key);
        }

        // Filter out the uneeded info for broker layer and return vec of missing
        let missing = missing_nodes
            .iter()
            .map(|(_, (rloc, ip))| (*rloc, *ip))
            .collect::<Vec<_>>();

        Ok(missing)
    }
}

/// The [`cli::OtMonitor`](cli/struct.OtMonitor.html) actor and
/// other external event handlers may use this type
#[derive(Message, Clone)]
#[rtype(result = "NodeRegResponse")]
pub struct Registration {
    pub rloc: Rloc,
    pub ip: Ipv6Addr,
    pub port: NodeRcvPort,
    #[allow(unused)]
    pub eui: Eui,
}

type NodeRegResponse = Result<(), OtMonitorError>;
impl Handler<Registration> for OtMonitor {
    type Result = NodeRegResponse;

    fn handle(&mut self, msg: Registration, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("Node rloc: {:?} node port {:?}", msg.rloc, msg.port);
        self.register_node(msg)
    }
}

/// Check for new nodes
#[derive(Message)]
#[rtype(result = "NewNodeResponse")]
pub struct CheckNewNode;
type NewNodeResponse = Result<Vec<(u16, Ipv6Addr)>, OtMonitorError>;

impl Handler<CheckNewNode> for OtMonitor {
    type Result = NewNodeResponse;

    fn handle(&mut self, _msg: CheckNewNode, _ctx: &mut Self::Context) -> Self::Result {
        let active_nodes = self.get_nodes()?;

        let new_nodes = active_nodes
            .iter()
            .filter_map(|(rloc, ip)| {
                if let Some(_found) = self
                    .nodes
                    .iter()
                    .find(|&(_r, i)| i.rloc == *rloc && i.ip == *ip)
                {
                    None
                } else {
                    Some((rloc, ip))
                }
            })
            .map(|p| (*p.0, *p.1))
            .collect();

        Ok(new_nodes)
    }
}

/// Check general network status, e.g. if OMR prefix has changed & needs updating
#[derive(Message)]
#[rtype(result = "MonitorNetworkResponse")]
pub struct MonitorNetworkStatus;
type MonitorNetworkResponse = Result<(), OtMonitorError>;

impl Handler<MonitorNetworkStatus> for OtMonitor {
    type Result = MonitorNetworkResponse;

    fn handle(&mut self, _msg: MonitorNetworkStatus, _ctx: &mut Self::Context) -> Self::Result {
        if self.check_addr_update_needed()? {
            self.update_addr()?;
        }
        Ok(())
    }
}

/// Get the OMR addr
#[derive(Message)]
#[rtype(result = "OmrResponse")]
pub struct OmrIp;
type OmrResponse = Result<Ipv6Addr, OtMonitorError>;

impl Handler<OmrIp> for OtMonitor {
    type Result = OmrResponse;

    fn handle(&mut self, _msg: OmrIp, _ctx: &mut Self::Context) -> Self::Result {
        self.get_omr_ip()
    }
}

/// Get a free port to use when trying to register a new node
#[derive(Message)]
#[rtype(result = "ReserveFreePortResponse")]
pub struct ReserveFreePort;
type ReserveFreePortResponse = Result<u16, OtMonitorError>;

impl Handler<ReserveFreePort> for OtMonitor {
    type Result = ReserveFreePortResponse;

    fn handle(&mut self, _msg: ReserveFreePort, _ctx: &mut Self::Context) -> Self::Result {
        self.get_free_port()
    }
}

/// Return the port if registration of new node needs retry
#[derive(Message)]
#[rtype(result = "ReturnFreePortResponse")]
pub struct ReturnFreePort(pub u16);
type ReturnFreePortResponse = ();

impl Handler<ReturnFreePort> for OtMonitor {
    type Result = ();

    fn handle(&mut self, msg: ReturnFreePort, _ctx: &mut Self::Context) -> Self::Result {
        self.return_port(msg.0)
    }
}
