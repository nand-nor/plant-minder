//! The responsibilities of this layer are the following:
//! 1. Maintain list of active children and their IPv6 addresses
//! 2. For each active child:
//!    a. Maintain active CoAP (observer client) status  
//!    b. Manage socket to receive sensor data
//!    c. Enqueue received data to event queue

use std::{net::Ipv6Addr, process::Command};

use ipnet::Ipv6Net;

mod broker;
mod monitor;

pub use broker::BrokerCoordinator;
pub use monitor::{OtMonitor, OtMonitorError};

pub trait OtClient: Send + Sync {
    fn get_child_ips(&self) -> Result<Vec<(String, Ipv6Addr)>, OtCliError>;
    fn get_omr_prefix(&self) -> Result<Ipv6Net, OtCliError>;
    fn get_omr_ip(&self) -> Result<Ipv6Addr, OtCliError>;
    fn get_ip_addrs(&self) -> Result<Vec<Ipv6Addr>, OtCliError>;
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OtCliError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Str utf8 parse Error")]
    StrParse(#[from] std::str::Utf8Error),
    #[error("AddrParse error")]
    AddrParse(#[from] ipnet::AddrParseError),
    #[error("OT Client Error {0}")]
    OtClientError(String),
}

pub struct OtCliClient;

// TODO: to avoid ExitStatus(unix_wait_status(256)) implement backoff + retries !
impl OtCliClient {
    pub fn get_children_from_cli(&self) -> Result<Vec<(String, Ipv6Addr)>, OtCliError> {
        let child_resp = Command::new("ot-ctl").arg("childip").output()?;

        if child_resp.status.success() {
            Ok(OtCliClient::parse_childip_output(
                std::str::from_utf8(&child_resp.stdout)?.to_string(),
            ))
        } else {
            Err(OtCliError::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed CLI Command: exit status {:?}", child_resp.status),
            )))
        }
    }

    fn parse_childip_output(res: String) -> Vec<(String, Ipv6Addr)> {
        let res = res.trim_end_matches("Done");
        let lines = res.split('\n').collect::<Vec<_>>();

        let res = lines
            .iter()
            .map(|l| {
                let idx = l.find(": ");
                if let Some(idx) = idx {
                    let (rloc, rem) = l.split_at(idx);
                    let rem = rem.trim_start_matches(": ");
                    let rem = rem.trim();
                    /*
                     if let Ok(ip) = rem.parse::<std::net::Ipv6Addr>() {
                        Some((rloc.to_string(), ip))
                    } else {
                        None
                    }
                     */
                    rem.parse::<Ipv6Addr>()
                        .ok()
                        .map(|ip| (rloc.to_string(), ip))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .iter()
            .flatten()
            .cloned()
            .collect();
        res
    }

    fn get_omr_prefix_from_cli(&self) -> Result<Ipv6Net, OtCliError> {
        let resp = Command::new("ot-ctl").arg("prefix").output()?;

        if resp.status.success() {
            OtCliClient::parse_prefix_output(std::str::from_utf8(&resp.stdout)?.to_string())
        } else {
            Err(OtCliError::OtClientError(format!(
                "Failed CLI Command: exit status {:?}",
                resp.status
            )))
        }
    }

    fn parse_prefix_output(res: String) -> Result<Ipv6Net, OtCliError> {
        let res = res.trim_end_matches("Done");
        let elems = res.split(' ').collect::<Vec<_>>();
        if elems.is_empty() {
            Err(OtCliError::OtClientError(
                "No prefix is currently set".to_string(),
            ))
        } else {
            Ok(elems[0].parse()?)
        }
    }

    fn get_ip_addrs_from_cli(&self) -> Result<Vec<Ipv6Addr>, OtCliError> {
        let resp = Command::new("ot-ctl").arg("ipaddr").output()?;

        if resp.status.success() {
            let res = std::str::from_utf8(&resp.stdout)?.to_string();
            let res = res.trim_end_matches("Done");
            let elems = res.split('\n').collect::<Vec<_>>();
            let ips = elems
                .iter()
                .map(|i| {
                    let i = i.trim();
                    i.parse::<std::net::Ipv6Addr>()
                })
                .collect::<Vec<_>>()
                .iter()
                .flatten()
                .cloned()
                .collect();
            Ok(ips)
        } else {
            Err(OtCliError::OtClientError(format!(
                "Failed CLI Command: exit status {:?}",
                resp.status
            )))
        }
    }

    pub fn get_omr_ip_addr_from_cli(&self) -> Result<Ipv6Addr, OtCliError> {
        let prefix = self.get_omr_prefix_from_cli()?;
        let prefix_addr = prefix.addr();
        let ips = self.get_ip_addrs_from_cli()?;
        if let Some(ip) = ips
            .iter()
            .find(|i| i.segments()[0] == prefix_addr.segments()[0])
        {
            Ok(*ip)
        } else {
            Err(OtCliError::OtClientError(
                "No matching prefix found".to_string(),
            ))
        }
    }
}

impl OtClient for OtCliClient {
    fn get_child_ips(&self) -> Result<Vec<(String, Ipv6Addr)>, OtCliError> {
        self.get_children_from_cli()
    }

    fn get_omr_prefix(&self) -> Result<Ipv6Net, OtCliError> {
        self.get_omr_prefix_from_cli()
    }

    fn get_omr_ip(&self) -> Result<Ipv6Addr, OtCliError> {
        self.get_omr_ip_addr_from_cli()
    }

    fn get_ip_addrs(&self) -> Result<Vec<Ipv6Addr>, OtCliError> {
        self.get_ip_addrs_from_cli()
    }
}

#[cfg(test)]
mod tests {
    use ipnet::Ipv6Net;
    use std::net::Ipv6Addr;

    use crate::OtCliClient;

    #[tokio::test]
    async fn check_cli_parse_child_ips() {
        let res = "c04f: fd1f:a298:dbd1:e329:1c45:9c98:b941:1a5a\r\nc04f: fde0:dc9c:b343:1:9b57:cf1a:c2d3:49d5\r\nDone\r\n".to_string();
        let ret = OtCliClient::parse_childip_output(res);
        assert_eq!(
            ret[1],
            (
                "co4f".to_string(),
                Ipv6Addr::from([0xfde0, 0xdc9c, 0xb343, 0x1, 0x9b57, 0xcf1a, 0xc2d3, 0x49d5])
            )
        );
    }

    #[tokio::test]
    async fn check_cli_parse_prefix() {
        let res = "fdc9:fdb2:9fe8:1::/64 paos low 4400\r\nDone".to_string();
        let ret: Ipv6Net = OtCliClient::parse_prefix_output(res)
            // .await
            .expect("Unable to get Ipv6Net");
        assert_eq!(ret, "fdc9:fdb2:9fe8:1::/64".parse().unwrap());
    }
}
