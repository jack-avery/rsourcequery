use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::error::SourceQueryError;

use crate::packet::{RequestPacket, ResponsePacket, PacketType, ServerInfo};

async fn send_recv(sock: &UdpSocket, packet: RequestPacket, timeout_dur: Duration) -> Result<ResponsePacket, SourceQueryError> {
    // sending
    timeout(timeout_dur, sock.send(&packet.pack()))
        .await?
        .map_err(SourceQueryError::SendError)?;

    // receiving packet
    let mut resp_buf: [u8; 1400] = [0u8; 1400];
    timeout(timeout_dur, sock.recv(&mut resp_buf))
        .await?
        .map_err(SourceQueryError::ReceiveError)?;

    ResponsePacket::unpack(resp_buf)
}

pub async fn query(host: &str, timeout_dur: Option<Duration>) -> Result<ServerInfo, SourceQueryError> {
    let timeout_dur: Duration = timeout_dur.unwrap_or(Duration::from_secs(5));

    // just arbitrarily bind any port, doesn't matter really
    let sock: UdpSocket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(SourceQueryError::FailedPortBind)?;

    // connecting
    timeout(timeout_dur, sock.connect(host))
        .await?
        .map_err(SourceQueryError::UnreachableHost)?;

    // sending initial packet
    let req_packet: RequestPacket = RequestPacket::new(None);
    let packet: ResponsePacket = send_recv(&sock, req_packet, timeout_dur).await?;

    // absolving challenge
    if packet.packet_type() == &PacketType::Challenge {
        let req_packet: RequestPacket = RequestPacket::new(packet.body());
        let packet: ResponsePacket = send_recv(&sock, req_packet, timeout_dur).await?;
        ServerInfo::parse(packet)
    // no challenge?
    } else {
        ServerInfo::parse(packet)
    }
}