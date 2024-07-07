use std::net::{Ipv6Addr, SocketAddr};
use futures::prelude::*;
use actix::{Actor, MailboxError};
use coap_lite::{CoapRequest, ObserveOption, RequestType};
use tokio::net::UdpSocket;

use crate::{
    monitor::{CheckNewChild, GetChildStatus, MonitorNetworkStatus, OmrIp},
    OtMonitor, OtMonitorError,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrokerCoordinatorError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("OT monitor Error")]
    OtMonError(#[from] OtMonitorError),
    #[error("Actix mailbox Error")]
    MailError(#[from] MailboxError),

    #[error("CoAP Msg Error")]
    CoAPMsgError(#[from] coap_lite::error::MessageError),

    #[error("AddrParse error")]
    AddrParse(#[from] std::net::AddrParseError),
}


pub struct BrokerCoordinator {

    monitor_handle: tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>,
    event_queue_handle: tokio::task::JoinHandle<Result<(), BrokerCoordinatorError>>, 

    socket: UdpSocket,
}

impl BrokerCoordinator {

    pub async fn coap_observer_register(omr_addr: Ipv6Addr, ip_addr: Ipv6Addr) -> Result<(), BrokerCoordinatorError> {

        let mut request: CoapRequest<SocketAddr> = CoapRequest::new();
        let mut buffer = [0u8; 512];
        // following https://datatracker.ietf.org/doc/html/rfc7641 observing resources in CoAP
        request.set_method(RequestType::Get);
        request.set_path("/soilmoisture");
        request.message.set_token(vec![0xfa, 0xce, 0xbe, 0xef]);
        request.set_observe_flag(ObserveOption::Register);
        let packet = request.message.to_bytes()?;

        let ip_w_port = format!("[{}]:1212", ip_addr);
        // fix this later
        let send_addr: std::net::SocketAddrV6 = ip_w_port.parse()?;

        let addr = format!("[{}]:1213", omr_addr.to_string());
        let addr: std::net::SocketAddrV6 = addr.parse()?;
    
        let send_socket = UdpSocket::bind(addr).await?;
    
        if let Err(e) = send_socket.send_to(&packet[..], send_addr).await {
            log::error!("Error sending: {e:}");
        }

        // allow retries in case the radio is currently idle
        // not currently enabling rx_on_when_idle, should only
        // be a couple seconds 1 min max worst case
        tokio::select!{
            Ok((mut len, mut from)) = send_socket.recv_from(&mut buffer) => {
                while len <= 0 {
                    // sleep a lil
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    if let Err(e) = send_socket.send_to(&packet[..], send_addr).await {
                        log::error!("Error sending: {e:}");
                    }
                    (len, from) = send_socket.recv_from(&mut buffer).await?
                }
                log::info!("Got a response from {from:}");

            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {

            }
        };
        
        Ok(())
    }

    
    pub async fn spawn_socket_listener(&mut self, socket: UdpSocket) {
        let handle = tokio::spawn(async move {
            let mut buffer = [0u8; 512];

            loop {

                let (len, src) = socket.recv_from(&mut buffer).await?; 
                    if len > 0 {
                        let mut moisture_s: [u8; 2] = [0u8; 2];
                        moisture_s.copy_from_slice(&buffer[..2]);
                        let moisture = u16::from_le_bytes(moisture_s);
                        let mut temp_s: [u8; 4] = [0u8; 4];
                        temp_s.copy_from_slice(&buffer[2..6]);
                        let temp = f32::from_le_bytes(temp_s);
            
                        println!("{:?} sent moisture: {:?} temp {:?}", src, moisture, temp);
                    }
                
            }
            Ok(())
        });

        self.event_queue_handle = handle;
    }

    pub async fn spawn_child_mon_task(&mut self, poll_interval: u64, ot_mon: OtMonitor) {
        
        let addr = ot_mon.start();
        let poll = tokio::time::Duration::from_secs(poll_interval);
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(poll).await;
                addr.send(MonitorNetworkStatus).await?.map_err(|e| {
                    log::error!("Error checking omr prefix {e:}");
                    e
                })?;

                if let Ok(children) = addr.send(CheckNewChild).await? {

                    if let Ok(omr_addr) = addr.send(OmrIp).await? {

                        futures::stream::iter(children)
                        .for_each(|ip| async move {
                            let res =  BrokerCoordinator::coap_observer_register(omr_addr.clone(), ip).await;
                            log::info!("Result: {:?}", res);
                        })
                        .await;

                    }
                } else {
                    break;
                }

                if let Ok(lost_children) = addr.send(GetChildStatus).await? {
                    // TODO need to handle this
                    if !lost_children.is_empty() {
                        log::warn!("Lost children {:?}", lost_children);
                    }
                } else {
                    break;
                }
            }
            //addr.terminate();
            //todo log somethin
            Ok(())
        });

        self.monitor_handle = handle;

    }
}
