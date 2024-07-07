use coap_lite::{
    CoapOption, CoapRequest, MessageClass, MessageType, ObserveOption, Packet, RequestType,
    ResponseType,
};
use pmindd;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use std::net::{Ipv6Addr, SocketAddr, UdpSocket};

use pmindb::{OtCliClient, OtMonitor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut request: CoapRequest<SocketAddr> = CoapRequest::new();

    // following https://datatracker.ietf.org/doc/html/rfc7641 observing resources in CoAP
    request.set_method(RequestType::Get);
    request.set_path("/soilmoisture");
    request.message.set_token(vec![0xfa, 0xce, 0xbe, 0xef]);
    request.set_observe_flag(ObserveOption::Register);
    let packet = request.message.to_bytes().unwrap();

    let mut ot_mon = OtMonitor::new(std::boxed::Box::new(OtCliClient));

    let omr_addr = ot_mon.get_omr_ip()?;
    let addr = format!("[{}]:1212", omr_addr.to_string());
    let addr: std::net::SocketAddrV6 = addr.parse()?;

    let socket = UdpSocket::bind(addr).unwrap();

    let children = ot_mon.get_children()?;
    let mut buffer = [0u8; 512];

    children.iter().for_each(|ip| {
        let ip_w_port = format!("[{}]:1212", ip.clone());
        // fix this later
        let send_addr: std::net::SocketAddrV6 = ip_w_port.parse().unwrap();

        let mut len = 0;
        // allow retries in case the radio is currently idle
        // not currently enabling rx_on_when_idle, should only
        // be a couple seconds 1 min max worst case
        while len <= 0 {
            if let Err(e) = socket.send_to(&packet[..], send_addr) {
                std::process::exit(1);
            }
            len = socket.recv(&mut buffer).unwrap();
        }
    });

    println!("Sent CoAP observe registration");

    loop {
        let (len, src) = socket.recv_from(&mut buffer).unwrap();
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
}
