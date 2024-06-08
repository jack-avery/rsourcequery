use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::error::SourceQueryError;

use crate::packet::{RequestPacket, ResponsePacket, PacketType};

use std::{ops::RangeInclusive, str};

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
    pub vac_enabled: bool
    //TODO: implement version, EDF (e.g. tags)
    // these are unimportant to my use case, so... later?
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

    /// Parse a [ResponsePacket] into its' corresponding [ServerInfo].
    /// This will probably panic if [ResponsePacket] is not [PacketType::Response].
    pub fn parse(packet: ResponsePacket) -> Result<ServerInfo, SourceQueryError> {
        let mut data: Vec<u8> = packet.body();
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

        let game_id_pair = &data[Self::GAME_ID_OFFSET];
        let game_id = ((game_id_pair[0] as u16) << 8) | (game_id_pair[1] as u16);
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
        ServerInfo::parse(packet)
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