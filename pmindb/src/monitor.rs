use crate::{OtCliError, OtClient};
use actix::prelude::*;

use std::{collections::HashMap, net::Ipv6Addr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OtMonitorError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("OT Client Error")]
    OtClientError(#[from] OtCliError),
}

pub struct OtMonitor {
    nodes: HashMap<String, Ipv6Addr>,
    addr: Ipv6Addr,
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

        Self {
            nodes: HashMap::default(),
            addr,
            ot_client,
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

    pub fn get_nodes(&self) -> Result<Vec<(String, Ipv6Addr)>, OtMonitorError> {
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
            .map(|(r, p)| (r.clone(), *p))
            .collect())
    }

    pub fn node_registered(&mut self, node: (String, Ipv6Addr)) -> Result<(), OtMonitorError> {
        log::debug!("Registering node {} : {}", node.0, node.1);
        self.nodes
            .entry(node.0)
            .and_modify(|a| *a = node.1)
            .or_insert(node.1);
        Ok(())
    }

    #[allow(unused)]
    pub fn evict_node(&mut self, node: Ipv6Addr) -> Result<String, OtMonitorError> {
        todo!()
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

type NodeStatusResponse = Result<Vec<(String, Ipv6Addr)>, OtMonitorError>;

/// Check for new nodes
#[derive(Message)]
#[rtype(result = "NewNodeResponse")]
pub struct CheckNewNode;
type NewNodeResponse = Result<Vec<(String, Ipv6Addr)>, OtMonitorError>;

/// Check general network status, e.g. if OMR prefix has changed & needs updating
#[derive(Message)]
#[rtype(result = "MonitorNetworkResponse")]
pub struct MonitorNetworkStatus;
type MonitorNetworkResponse = Result<(), OtMonitorError>;

/// Get the OMR addr
#[derive(Message)]
#[rtype(result = "OmrResponse")]
pub struct OmrIp;
type OmrResponse = Result<Ipv6Addr, OtMonitorError>;

/// Indicate child successfully registered
#[derive(Message)]
#[rtype(result = "NodeRegResponse")]
pub struct NodeRegistered(pub (String, Ipv6Addr));

type NodeRegResponse = Result<(), OtMonitorError>;

impl Handler<GetNodeStatus> for OtMonitor {
    type Result = NodeStatusResponse;

    fn handle(&mut self, _msg: GetNodeStatus, _ctx: &mut Self::Context) -> Self::Result {
        let active_nodes = self.get_nodes()?;

        // Iterate through the hashmap of currently registered nodes; if any are not in the active node list then
        // they are missing
        let missing = self
            .nodes
            .iter()
            .filter_map(|(rloc, ip)| {
                if let Some(_found) = active_nodes.iter().find(|(r, i)| r == rloc && i == ip) {
                    None
                } else {
                    Some((rloc, ip))
                }
            })
            .map(|p| (p.0.clone(), *p.1))
            .collect();
        Ok(missing)
    }
}

impl Handler<NodeRegistered> for OtMonitor {
    type Result = NodeRegResponse;

    fn handle(&mut self, msg: NodeRegistered, _ctx: &mut Self::Context) -> Self::Result {
        self.node_registered(msg.0)
    }
}

impl Handler<CheckNewNode> for OtMonitor {
    type Result = NewNodeResponse;

    fn handle(&mut self, _msg: CheckNewNode, _ctx: &mut Self::Context) -> Self::Result {
        let active_nodes = self.get_nodes()?;

        let new_nodes = active_nodes
            .iter()
            .filter_map(|(rloc, ip)| {
                if let Some(_found) = self.nodes.iter().find(|&(r, i)| r == rloc && i == ip) {
                    None
                } else {
                    Some((rloc, ip))
                }
            })
            .map(|p| (p.0.clone(), *p.1))
            .collect();

        Ok(new_nodes)
    }
}

impl Handler<MonitorNetworkStatus> for OtMonitor {
    type Result = MonitorNetworkResponse;

    fn handle(&mut self, _msg: MonitorNetworkStatus, _ctx: &mut Self::Context) -> Self::Result {
        if self.check_addr_update_needed()? {
            self.update_addr()?;
        }
        Ok(())
    }
}

impl Handler<OmrIp> for OtMonitor {
    type Result = OmrResponse;

    fn handle(&mut self, _msg: OmrIp, _ctx: &mut Self::Context) -> Self::Result {
        self.get_omr_ip()
    }
}
