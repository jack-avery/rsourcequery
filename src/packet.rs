use std::ops::RangeInclusive;

use crate::error::SourceQueryError;

#[derive(Debug, PartialEq, Eq)]
pub enum PacketHeader {
    Single,
    Split,
}

/// Convert an i32 into a [PacketHeader].
impl TryInto<PacketHeader> for i32 {
    type Error = SourceQueryError;

    fn try_into(self) -> Result<PacketHeader, Self::Error> {
        match self {
            -1 => Ok(PacketHeader::Single),
            -2 => Ok(PacketHeader::Split),
            n => Err(SourceQueryError::UnknownPacketHeader(n)),
        }
    }
}

/// For packing a [PacketHeader] into a packet in [RequestPacket::pack].
impl PacketHeader {
    pub fn to_le_bytes(&self) -> [u8; 4] {
        let type_value: i32 = match self {
            PacketHeader::Single => -1,
            PacketHeader::Split => -2,
        };
        type_value.to_le_bytes()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PacketType {
    /// A2S_INFO Request -- https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO
    ///
    /// Retrieves information about the server including, but not limited to:
    /// its name, the map currently being played, and the number of players.
    Request,
    /// S2C_CHALLENGE
    ///
    /// the server may reply with a challenge to the client using S2C_CHALLENGE
    /// ('A' or 0x41). In that case, the client should repeat the request by appending the challenge number.
    Challenge,
    /// A2S_INFO Response Packet -- https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO
    ///
    /// To be parsed by [ServerInfo::parse].
    Response,
}

/// Convert a u8 into a [PacketType].
impl TryInto<PacketType> for u8 {
    type Error = SourceQueryError;

    fn try_into(self) -> Result<PacketType, Self::Error> {
        match self {
            84 => Ok(PacketType::Request),
            65 => Ok(PacketType::Challenge),
            73 => Ok(PacketType::Response),
            n => Err(SourceQueryError::UnknownPacketType(n)),
        }
    }
}

/// For packing a [PacketType] into a packet in [RequestPacket::pack].
impl PacketType {
    pub fn to_byte(&self) -> u8 {
        match self {
            PacketType::Request => 84, // 0x54
            PacketType::Challenge => 65, // 0x41
            PacketType::Response => 73 // 0x49
        }
    }
}

/// According to the Valve wiki, Source query responses use 1400 bytes + IP/UDP headers.
/// The only game found violating this is Rust, but we're not using this for Rust... right?
pub type RawPacket = [u8; 1400];

#[derive(Debug, PartialEq, Eq)]
pub struct RequestPacket {
    packet_header: PacketHeader,
    packet_type: PacketType,
    body: String,
    challenge: Option<Vec<u8>>
}

impl RequestPacket {
    pub fn new(challenge: Option<Vec<u8>>) -> Self {
        RequestPacket {
            packet_header: PacketHeader::Single,
            packet_type: PacketType::Request,
            body: "Source Engine Query".to_owned(), // honestly, jank
            challenge
        }
    }

    /// Serializes a request packet into an array of bytes.
    pub fn pack(&self) -> Vec<u8> {
        // packet structure: header, type, body, terminator (and challenge)
        let mut payload: Vec<u8> = Vec::<u8>::new();
        payload.extend_from_slice(&self.packet_header().to_le_bytes());
        payload.extend_from_slice(&[self.packet_type().to_byte()]);
        payload.extend_from_slice(self.body().as_bytes());
        // null terminate the body
        payload.extend_from_slice(&[0]);
        if let Some(c) = &self.challenge {
            payload.extend_from_slice(c);
        }
        
        payload
    }

    pub fn packet_header(&self) -> &PacketHeader {
        &self.packet_header
    }

    pub fn packet_type(&self) -> &PacketType {
        &self.packet_type
    }

    pub fn body(&self) -> String {
        self.body.clone()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResponsePacket {
    packet_header: PacketHeader,
    id: Option<i32>,
    total: Option<u8>,
    number: Option<u8>,
    size: Option<usize>,
    unpacked_size: Option<u32>,
    packet_type: PacketType,
    body: Vec<u8>
}

impl ResponsePacket {
    const HEADER_RANGE: RangeInclusive<usize> = 0..=3;

    const SINGLE_TYPE_OFFSET: usize = 4;
    const SINGLE_BODY_OFFSET: usize = 5;
    const CHALLENGE_BODY: RangeInclusive<usize> = 5..=8;

    const SPLIT_ID_RANGE: RangeInclusive<usize> = 4..=7;
    const SPLIT_TOTAL_OFFSET: usize = 8;
    const SPLIT_NUMBER_OFFSET: usize = 9;

    /// Deserializes an incoming packet, splitting it up into headers and body.
    pub fn unpack(incoming: RawPacket) -> Result<Self, SourceQueryError> {
        let raw_header = &incoming[Self::HEADER_RANGE];
        let raw_header = i32::from_le_bytes(raw_header.try_into()?);
        let packet_header: PacketHeader = raw_header.try_into()?;

        match packet_header {
            PacketHeader::Single => {
                let raw_type = &incoming[Self::SINGLE_TYPE_OFFSET];
                let packet_type: PacketType = raw_type.to_owned().try_into()?;
                
                let raw_body = if packet_type == PacketType::Challenge {
                    &incoming[Self::CHALLENGE_BODY]
                } else {
                    &incoming[Self::SINGLE_BODY_OFFSET..]
                };
                let body = raw_body.to_vec();
                
                let packet = ResponsePacket {
                    packet_header,
                    id: None,
                    total: None,
                    number: None,
                    size: None,
                    unpacked_size: None,
                    packet_type,
                    body
                };

                Ok(packet)
            },
            //TODO: handle split response packets
            PacketHeader::Split => unimplemented!(),
        }
    }

    pub fn packet_header(&self) -> &PacketHeader {
        &self.packet_header
    }

    pub fn packet_type(&self) -> &PacketType {
        &self.packet_type
    }

    pub fn body(&self) -> Vec<u8> {
        self.body.clone()
    }
}
