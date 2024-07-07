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
    children: HashMap<String, Ipv6Addr>,
    addr: Ipv6Addr,
    ot_client: Box<dyn OtClient>,
}

impl OtMonitor {
    pub fn new(ot_client: Box<dyn OtClient>) -> Self {
        let addr = {
            if let Ok(addr) = ot_client.get_omr_ip() {
                addr
            } else {
                // Update it later
                Ipv6Addr::from([0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1])
            }
        };

        Self {
            children: HashMap::default(),
            addr,
            ot_client,
        }
    }

    pub fn get_omr_ip(&self) -> Result<Ipv6Addr, OtMonitorError> {
        Ok(self.ot_client.get_omr_ip()?)
    }

    pub fn check_addr_update_needed(&mut self) -> Result<bool, OtMonitorError> {
        Ok(self.addr.is_loopback()
            || self.addr.segments()[0] != self.ot_client.get_omr_prefix()?.addr().segments()[0])
    }

    pub fn update_addr(&mut self) -> Result<(), OtMonitorError> {
        self.addr = self.ot_client.get_omr_ip()?;
        Ok(())
    }

    pub fn get_current_active_children(&self) -> Result<Vec<Ipv6Addr>, OtMonitorError> {
        Ok(self
            .ot_client
            .get_child_ips()?
            .iter()
            .map(|(_, ip)| ip.clone())
            .collect())
    }

    pub fn get_current_tracked_children(&self) -> Vec<Ipv6Addr> {
        self.children.clone().into_values().collect()
    }

    pub fn get_children(&mut self) -> Result<Vec<Ipv6Addr>, OtMonitorError> {
        let children = self.ot_client.get_child_ips()?;
        self.inspect(children)
    }

    fn inspect(
        &mut self,
        children: Vec<(String, Ipv6Addr)>,
    ) -> Result<Vec<Ipv6Addr>, OtMonitorError> {
        let new_children: Vec<Ipv6Addr> = children
            .iter()
            .map(|(rloc, addr)| {
                if addr.segments()[0] == self.addr.segments()[0] {
                    // Only push addrs that match the addr scope (OMR)
                    self.children
                        .entry(rloc.clone())
                        .and_modify(|a| *a = addr.clone())
                        .or_insert(*addr);
                    Some(addr.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .iter()
            .flatten()
            .cloned()
            .collect();
        return Ok(new_children);
    }
}

impl Actor for OtMonitor {
    type Context = Context<Self>;
}

/// Check to see if any previously registered children have
/// fallen off the network
#[derive(Message)]
#[rtype(result = "ChildStatusResponse")]
pub struct GetChildStatus;

type ChildStatusResponse = Result<Vec<String>, OtMonitorError>;

/// Check for new children
#[derive(Message)]
#[rtype(result = "NewChildResponse")]
pub struct CheckNewChild;
type NewChildResponse = Result<Vec<Ipv6Addr>, OtMonitorError>;

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


impl Handler<GetChildStatus> for OtMonitor {
    type Result = ChildStatusResponse;

    fn handle(&mut self, _msg: GetChildStatus, _ctx: &mut Self::Context) -> Self::Result {
        todo!()
    }
}

impl Handler<CheckNewChild> for OtMonitor {
    type Result = NewChildResponse;

    fn handle(&mut self, _msg: CheckNewChild, _ctx: &mut Self::Context) -> Self::Result {
        self.get_children()
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

