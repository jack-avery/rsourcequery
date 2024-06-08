mod packet;
mod error;

use crate::error::SourceQueryError;

use crate::packet::{ResponsePacket, ServerInfo};

use std::fs::{read};

#[tokio::main]
async fn main() -> Result<(), SourceQueryError> {
    let raw_packet: [u8; 2048] = read("test_packet").unwrap().try_into().unwrap();
    let packet = ResponsePacket::unpack(raw_packet).expect("failed to parse packet");
    let info = ServerInfo::parse(packet).expect("failed to parse server info");
    dbg!(info);

    Ok(())
}