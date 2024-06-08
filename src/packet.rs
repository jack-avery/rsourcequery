use std::{ops::RangeInclusive, str};

use byteorder::{LittleEndian, ReadBytesExt};

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
    /// A2S_INFO -- https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO
    /// Retrieves information about the server including, but not limited to:
    /// its name, the map currently being played, and the number of players.
    Request,
    /// S2C_CHALLENGE
    /// the server may reply with a challenge to the client using S2C_CHALLENGE
    /// ('A' or 0x41). In that case, the client should repeat the request by appending the challenge number.
    Challenge,
    Response,
}

/// Convert an i32 into a [PacketType].
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

impl PacketType {
    /// Valve tells us that the fields in the header of a rcon packet are all
    /// signed 32-bit integers in low-endian, so we can easily convert like so.
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
pub type RawPacket = [u8; 2048];

#[derive(Debug)]
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
        // packet structure: size, ID, type, body, terminator
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

/// Low level implementation of a rcon packet.
#[derive(Debug)]
pub struct ResponsePacket {
    packet_header: PacketHeader,
    id: Option<i32>,
    total: Option<u8>,
    number: Option<u8>,
    size: Option<usize>,
    unpacked_size: Option<u32>,
    packet_type: PacketType,
    body: Option<Vec<u8>>
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

                if packet_type == PacketType::Challenge {
                    let raw_challenge = &incoming[Self::CHALLENGE_BODY];
                    let body = Some(raw_challenge.to_vec());
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
                    return Ok(packet);
                }
                
                let raw_body = &incoming[Self::SINGLE_BODY_OFFSET..];
                let body = Some(raw_body.to_vec());
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
            PacketHeader::Split => todo!(),
        }
    }

    pub fn packet_header(&self) -> &PacketHeader {
        &self.packet_header
    }

    pub fn packet_type(&self) -> &PacketType {
        &self.packet_type
    }

    pub fn body(&self) -> Option<Vec<u8>> {
        self.body.clone()
    }
}

#[derive(Debug)]
pub struct ServerInfo {
    protocol: u8,
    hostname: String,
    map: String,
    folder: String,
    game: String,
    game_id: u16,
    players: u8,
    maxplayers: u8,
    bots: u8,
    server_type: char,
    server_env: char,
    password_protected: bool,
    vac_enabled: bool
}

impl ServerInfo {
    const GAME_ID_OFFSET: RangeInclusive<usize> = 0..=1;
    const PLAYERS_OFFSET: usize = 2;
    const MAXPLAYERS_OFFSET: usize = 3;
    const BOTS_OFFSET: usize = 4;
    const TYPE_OFFSET: usize = 5;
    const ENV_OFFSET: usize = 6;
    const PASSWORD_PROTECTED_OFFSET: usize = 7;
    const VAC_ENABLED_OFFSET: usize = 8;

    pub fn parse(packet: ResponsePacket) -> Result<ServerInfo, SourceQueryError> {
        let mut data: Vec<u8> = packet.body.expect("empty packet??????");
        let protocol: u8 = data.remove(0);
        
        //TODO: improve string handling (resolve offsets?)
        let mut hostname_buf: Vec<u8> = Vec::new();
        let mut c: u8;
        loop {
            c = data.remove(0);
            if c == 0 {
                break;
            }
            hostname_buf.push(c);
        }
        let hostname: String = str::from_utf8(&hostname_buf)?.to_string();

        let mut map_buf: Vec<u8> = Vec::new();
        loop {
            c = data.remove(0);
            if c == 0 {
                break;
            }
            map_buf.push(c);
        }
        let map: String = str::from_utf8(&map_buf)?.to_string();

        let mut folder_buf: Vec<u8> = Vec::new();
        loop {
            c = data.remove(0);
            if c == 0 {
                break;
            }
            folder_buf.push(c);
        }
        let folder: String = str::from_utf8(&folder_buf)?.to_string();

        let mut game_buf: Vec<u8> = Vec::new();
        loop {
            c = data.remove(0);
            if c == 0 {
                break;
            }
            game_buf.push(c);
        }
        let game: String = str::from_utf8(&game_buf)?.to_string();

        // string handling is done, so we can just make this a slice
        let data: &[u8] = data.as_slice();

        let mut game_id_pair = &data[Self::GAME_ID_OFFSET];
        let game_id = game_id_pair.read_u16::<LittleEndian>().expect("huh??");
        let players = data[Self::PLAYERS_OFFSET];
        let maxplayers = data[Self::MAXPLAYERS_OFFSET];
        let bots = data[Self::BOTS_OFFSET];
        let server_type = char::from(data[Self::TYPE_OFFSET]);
        let server_env = char::from(data[Self::ENV_OFFSET]);
        let password_protected = data[Self::PASSWORD_PROTECTED_OFFSET] == 1;
        let vac_enabled = data[Self::VAC_ENABLED_OFFSET] == 1;

        Ok(ServerInfo {
            protocol,
            hostname,
            map,
            folder,
            game,
            game_id,
            players,
            maxplayers,
            bots,
            server_type,
            server_env,
            password_protected,
            vac_enabled
        })
    }
}