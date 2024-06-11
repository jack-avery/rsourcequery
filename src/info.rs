use std::time::Duration;

use bitvec::prelude::*;
use bitvec::view::BitView;

use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::error::SourceQueryError;

use crate::packet::{RequestPacket, ResponsePacket, PacketType};

/// Server information as obtained by [query].
#[derive(Debug)]
pub struct ServerInfo {
    /// A2S_INFO protocol version
    pub protocol: u8,
    /// Server hostname
    pub hostname: String,
    /// Current map
    pub map: String,
    /// Location of server files
    pub folder: String,
    /// Name of game
    pub game: String,
    /// Steam ID of game
    pub game_id: u16,
    /// Current players
    pub players: u8,
    /// Max players
    pub maxplayers: u8,
    /// Current bots
    pub bots: u8,
    /// Server type:
    /// - `d`: Dedicated
    /// - `l`: Listen (non-dedicated)
    /// - `p`: SourceTV relay (proxy)
    pub server_type: char,
    /// Server environment:
    /// - `l`: Linux
    /// - `w`: Windows
    /// - `o`: Mac
    pub server_env: char,
    /// Is the server password protected?
    pub password_protected: bool,
    /// Is the server VAC enabled?
    pub vac_enabled: bool,
    /// Version of the game installed on the server
    pub version: String,
    /// If present, this specifies which additional data fields will be included.
    pub edf: u8,
    /// Server port (EDF 0x80)
    pub port: Option<u16>,
    /// Server Steam ID (EDF 0x40)
    pub server_steam_id: Option<u64>,
    /// STV Port (EDF 0x20)
    pub stv_port: Option<u16>,
    /// STV Name (EDF 0x20)
    pub stv_name: Option<String>,
    /// Keywords (EDF 0x10)
    pub keywords: Option<Vec<String>>,
    /// Server Game ID (EDF 0x01)
    pub server_game_id: Option<u64>
}

impl ServerInfo {
    /// Get the value of a null-terminated string
    /// with index 0 at `offset` in an array of bytes.
    /// 
    /// Mutates `offset` to the index after the null-termination byte.
    fn get_string(data: &[u8], offset: &mut usize) -> Result<String, SourceQueryError> {
        let start_offset: usize = *offset;
        let mut end_offset: usize = *offset;

        while let Some(c) = data.get(end_offset) {
            end_offset += 1;
            if c == &0u8 {
                break;
            }
        }
        *offset = end_offset;

        Ok(std::str::from_utf8(&data[start_offset..end_offset-1])?.to_string())
    }

    /// Get the [u8] at index `offset` from `data`.
    /// 
    /// Mutates `offset` to the index after the byte.
    fn get_u8(data: &[u8], offset: &mut usize) -> u8 {
        let byte: u8 = data[*offset];
        *offset += 1;
        byte
    }

    /// Get 2 bytes (as a [u16]) at index `offset` from `data`.
    /// 
    /// Mutates `offset` to the index after the bytes.
    fn get_u16(data: &[u8], offset: &mut usize) -> u16 {
        let bytes: &[u8] = &data[*offset..=*offset + 1];
        *offset += 2;
        ((bytes[1] as u16) << 8) | (bytes[0] as u16)
    }

    /// Get 8 bytes (as a [u64]) at index `offset` from `data`.
    /// 
    /// Mutates `offset` to the index after the bytes.
    fn get_u64(data: &[u8], offset: &mut usize) -> u64 {
        let bytes: &[u8] = &data[*offset..*offset + 9];
        *offset += 8;
        ((bytes[7] as u64) << 56) |
        ((bytes[6] as u64) << 48) |
        ((bytes[5] as u64) << 40) |
        ((bytes[4] as u64) << 32) |
        ((bytes[3] as u64) << 24) |
        ((bytes[2] as u64) << 16) |
        ((bytes[1] as u64) << 8) |
        (bytes[0] as u64)
    }

    /// Parse a [ResponsePacket] into its' corresponding [ServerInfo].
    pub fn parse(packet: ResponsePacket) -> Result<ServerInfo, SourceQueryError> {
        if packet.packet_type() != &PacketType::Response {
            return Err(SourceQueryError::AttemptParseEmptyPacket());
        }

        let data: &Vec<u8> = &packet.body();
        let mut offset: usize = 0;

        let protocol = Self::get_u8(data, &mut offset);
        let hostname = Self::get_string(data, &mut offset)?;
        let map = Self::get_string(data, &mut offset)?;
        let folder = Self::get_string(data, &mut offset)?;
        let game = Self::get_string(data, &mut offset)?;
        let game_id = Self::get_u16(data, &mut offset);
        let players = Self::get_u8(data, &mut offset);
        let maxplayers = Self::get_u8(data, &mut offset);
        let bots = Self::get_u8(data, &mut offset);
        let server_type = char::from(Self::get_u8(data, &mut offset));
        let server_env = char::from(Self::get_u8(data, &mut offset));
        let password_protected = Self::get_u8(data, &mut offset) == 1;
        let vac_enabled = Self::get_u8(data, &mut offset) == 1;
        let version = Self::get_string(data, &mut offset)?;

        let edf = Self::get_u8(data, &mut offset);
        let edf_bitfield = edf.view_bits::<Msb0>();

        // 0x80 (Port)
        let port: Option<u16> = match edf_bitfield[0] {
            true => Some(Self::get_u16(data, &mut offset)),
            false => None,
        };
        // 0x40 (Server Steam ID)
        let server_steam_id: Option<u64> = match edf_bitfield[1] {
            true => Some(Self::get_u64(data, &mut offset)),
            false => None
        };
        // 0x20 (STV Port & Name)
        let stv_port: Option<u16>;
        let stv_name: Option<String>;
        if edf_bitfield[2] {
            stv_port = Some(Self::get_u16(data, &mut offset));
            stv_name = Some(Self::get_string(data, &mut offset)?);
        } else {
            stv_port = None;
            stv_name = None;
        }
        // 0x10 (Keywords)
        let keywords: Option<Vec<String>> = match edf_bitfield[3] {
            true => Some(
                Self::get_string(data, &mut offset)?
                    .split(',')
                    .map(|k| k.to_owned())
                    .collect()
            ),
            false => None
        };
        // 0x01 (GameID)
        let server_game_id: Option<u64> = match edf_bitfield[7] {
            true => Some(Self::get_u64(data, &mut offset)),
            false => None
        };

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
            vac_enabled,
            version,
            edf,
            port,
            server_steam_id,
            stv_port,
            stv_name,
            keywords,
            server_game_id
        })
    }
}

/// Query `host` with the Source Query Protocol A2S_INFO query.
/// 
/// If `timeout_dur` is `Some(Duration)`, each `timeout()` will use `timeout_dur`.
/// The default is 5 seconds if `timeout_dur` is `None`.
/// 
/// Note that this timeout duration can occur 3 times (5 if challenged):
/// - On socket connect
/// - On packet send
/// - On packet receive
/// - Twice more on another send and receive, if challenged
/// 
/// Example usage:
/// ```
/// let host: &str = "nyc-1.us.uncletopia.com:27015"; // Uncletopia New York City 4
/// let info: ServerInfo = query(host, None).await?;
/// ```
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
        let req_packet: RequestPacket = RequestPacket::new(Some(packet.body()));
        let packet: ResponsePacket = send_recv(&sock, req_packet, timeout_dur).await?;
        if packet.packet_type() == &PacketType::Response {
            ServerInfo::parse(packet)
        } else {
            Err(SourceQueryError::FussyHost(host.to_owned()))
        }
    // no challenge?
    } else {
        ServerInfo::parse(packet)
    }
}

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