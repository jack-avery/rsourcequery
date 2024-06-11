use std::time::Duration;

use bitvec::prelude::*;
use bitvec::view::BitView;

use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::error::SourceQueryError;

use crate::packet::{RequestPacket,ResponsePacket,PacketType};
use crate::parse::{get_string, get_u8, get_u16, get_u64};

/// Default timeout duration if using [`query`] as opposed to [`query_timeout_duration`]
const DEFAULT_TIMEOUT_SECS: u64 = 5;

/// [`ServerInfo`] - server information as obtained by [`query`] or [`query_timeout_duration`]
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
    /// Parse a [`ResponsePacket`] into its' corresponding [`ServerInfo`].
    pub fn parse(packet: ResponsePacket) -> Result<ServerInfo, SourceQueryError> {
        if packet.packet_type() != &PacketType::Response {
            return Err(SourceQueryError::AttemptParseEmptyPacket());
        }

        let data: &Vec<u8> = &packet.body();
        let mut offset: usize = 0;

        let protocol = get_u8(data, &mut offset);
        let hostname = get_string(data, &mut offset)?;
        let map = get_string(data, &mut offset)?;
        let folder = get_string(data, &mut offset)?;
        let game = get_string(data, &mut offset)?;
        let game_id = get_u16(data, &mut offset);
        let players = get_u8(data, &mut offset);
        let maxplayers = get_u8(data, &mut offset);
        let bots = get_u8(data, &mut offset);
        let server_type = char::from(get_u8(data, &mut offset));
        let server_env = char::from(get_u8(data, &mut offset));
        let password_protected = get_u8(data, &mut offset) == 1;
        let vac_enabled = get_u8(data, &mut offset) == 1;
        let version = get_string(data, &mut offset)?;

        let edf = get_u8(data, &mut offset);
        let edf_bitfield = edf.view_bits::<Msb0>();

        // 0x80 (Port)
        let port: Option<u16> = match edf_bitfield[0] {
            true => Some(get_u16(data, &mut offset)),
            false => None,
        };
        // 0x10 (Server Steam ID)
        let server_steam_id: Option<u64> = match edf_bitfield[3] {
            true => Some(get_u64(data, &mut offset)),
            false => None
        };
        // 0x40 (STV Port & Name)
        let stv_port: Option<u16>;
        let stv_name: Option<String>;
        if edf_bitfield[1] {
            stv_port = Some(get_u16(data, &mut offset));
            stv_name = Some(get_string(data, &mut offset)?);
        } else {
            stv_port = None;
            stv_name = None;
        }
        // 0x20 (Keywords)
        let keywords: Option<Vec<String>> = match edf_bitfield[2] {
            true => Some(
                get_string(data, &mut offset)?
                    .split(',')
                    .map(|k| k.to_owned())
                    .collect()
            ),
            false => None
        };
        // 0x01 (GameID)
        let server_game_id: Option<u64> = match edf_bitfield[7] {
            true => Some(get_u64(data, &mut offset)),
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

/// Query `host` with the Source Query Protocol A2S_INFO query,
/// with the default timeout duration of 5 seconds.
/// 
/// Note that this timeout duration can occur 3 times (5 if challenged):
/// - On socket connect
/// - On packet send
/// - On packet receive
/// - Twice more on another send and receive, if challenged
/// 
/// Example usage:
/// ```
/// let host: &str = "nyc-1.us.uncletopia.com:27015"; // Uncletopia New York City 1
/// let info: ServerInfo = query(host).await?;
/// ```
/// 
/// https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO
pub async fn query(host: &str) -> Result<ServerInfo, SourceQueryError> {
    query_timeout_duration(host, Duration::from_secs(DEFAULT_TIMEOUT_SECS)).await
}

/// Query `host` with the Source Query Protocol A2S_INFO query,
/// with a timeout duration of `timeout_dur`.
/// 
/// Note that this timeout duration can occur 3 times (5 if challenged):
/// - On socket connect
/// - On packet send
/// - On packet receive
/// - Twice more on another send and receive, if challenged
/// 
/// Example usage:
/// ```
/// let host: &str = "nyc-1.us.uncletopia.com:27015"; // Uncletopia New York City 1
/// let info: ServerInfo = query_timeout_duration(host, Duration::from_secs(2)).await?;
/// ```
/// 
/// https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO
pub async fn query_timeout_duration(host: &str, timeout_dur: Duration) -> Result<ServerInfo, SourceQueryError> {
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