/// Mod for different impls for interfacing with otbr-agent
/// currently only ot-ctl is implemented, soon to implement
/// something safer like DBus
mod cli;
use crate::Rloc;
pub use cli::OtCliClient;

use ipnet::Ipv6Net;
use std::net::Ipv6Addr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OtClientError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Str utf8 parse Error")]
    StrParse(#[from] std::str::Utf8Error),
    #[error("AddrParse error")]
    AddrParse(#[from] ipnet::AddrParseError),
    #[error("OT Client Error {0}")]
    OtClientErr(String),
}

/// Trait to allow different implementations for interfacing with the
/// otbr-agent
pub trait OtClient: Send + Sync {
    fn get_child_ips(&self) -> Result<Vec<(Rloc, Ipv6Addr)>, OtClientError>;
    fn get_omr_prefix(&self) -> Result<Ipv6Net, OtClientError>;
    fn get_omr_ip(&self) -> Result<Ipv6Addr, OtClientError>;
    #[allow(unused)]
    fn get_ip_addrs(&self) -> Result<Vec<Ipv6Addr>, OtClientError>;
}
