use pmindd;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use coap_lite::{
    CoapOption, CoapRequest, MessageClass, MessageType, Packet, RequestType, ResponseType, ObserveOption
};


use std::net::{SocketAddr, UdpSocket};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let mut request: CoapRequest<SocketAddr> = CoapRequest::new();

    // following https://datatracker.ietf.org/doc/html/rfc7641 observing resources in CoAP
    request.set_method(RequestType::Get);
    request.set_path("/soilmoisture");
    request.message.set_token(vec![0xfa,0xce,0xbe,0xef]);
    request.set_observe_flag(ObserveOption::Register);
    let packet = request.message.to_bytes().unwrap();

    let addr: std::net::SocketAddrV6 = "[fde0:dc9c:b343:1:4985:c57f:1f2d:e229]:1212".parse()?; 
    let socket = UdpSocket::bind(addr).unwrap();
    let send_addr: std::net::SocketAddrV6 = "[fde0:dc9c:b343:1:9b57:cf1a:c2d3:49d5]:1212".parse()?;

    let mut buffer = [0u8; 512];

    let mut len = 0;
    // allow retries in case the radio is currently idle 
    // not currently enabling rx_on_when_idle, should only 
    // be a couple seconds 1 min max
    while len <= 0 {
        if let Err(e) = socket.send_to(&packet[..], send_addr) {
            std::process::exit(1);
        }
        len = socket.recv(&mut buffer).unwrap();
    }

    println!("Sent CoAP observe registration");

    loop {

        let len = socket.recv(&mut buffer).unwrap();
        if len > 0 {
            let mut moisture_s: [u8; 2] = [0u8; 2];
            moisture_s.copy_from_slice(&buffer[..2]);
            let moisture = u16::from_le_bytes(moisture_s);
            let mut temp_s: [u8; 4] = [0u8; 4];
            temp_s.copy_from_slice(&buffer[2..6]);
            let temp = f32::from_le_bytes(temp_s);

            println!("Moisture: {:?} temp {:?}", moisture, temp);

        }
    }

    Ok(())
}