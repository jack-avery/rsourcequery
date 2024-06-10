use std::time::Duration;

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
    pub vac_enabled: bool
    //TODO: implement version, EDF (e.g. tags)
    // these are unimportant to my use case, so... later?
}

impl ServerInfo {
    const GAME_ID_SIZE: usize = 2;

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

        Ok(std::str::from_utf8(&data[start_offset..end_offset])?.to_string())
    }

    /// Get the byte at index `offset` from a `data`.
    /// 
    /// Mutates `offset` to the index after the byte.
    fn get_byte(data: &[u8], offset: &mut usize) -> u8 {
        let byte: u8 = data[*offset];
        *offset += 1;
        byte
    }

    /// Get `amount` bytes at index `offset` from `data`.
    /// 
    /// Mutates `offset` to the index after the bytes.
    fn get_bytes(data: &[u8], offset: &mut usize, amount: usize) -> Vec<u8> {
        let start_offset: usize = *offset;
        *offset += amount;
        data[start_offset..*offset].to_vec()
    }

    /// Parse a [ResponsePacket] into its' corresponding [ServerInfo].
    pub fn parse(packet: ResponsePacket) -> Result<ServerInfo, SourceQueryError> {
        if packet.packet_type() != &PacketType::Response {
            return Err(SourceQueryError::AttemptParseEmptyPacket());
        }

        let data: &Vec<u8> = &packet.body();
        let mut offset: usize = 0;

        let protocol = Self::get_byte(data, &mut offset);

        let hostname: String = Self::get_string(data, &mut offset)?;
        let map: String = Self::get_string(data, &mut offset)?;
        let folder: String = Self::get_string(data, &mut offset)?;
        let game: String = Self::get_string(data, &mut offset)?;

        let game_id_pair = Self::get_bytes(data, &mut offset, Self::GAME_ID_SIZE);
        let game_id = ((game_id_pair[0] as u16) << 8) | (game_id_pair[1] as u16);
        let players = Self::get_byte(data, &mut offset);
        let maxplayers = Self::get_byte(data, &mut offset);
        let bots = Self::get_byte(data, &mut offset);
        let server_type = char::from(Self::get_byte(data, &mut offset));
        let server_env = char::from(Self::get_byte(data, &mut offset));
        let password_protected = Self::get_byte(data, &mut offset) == 1;
        let vac_enabled = Self::get_byte(data, &mut offset) == 1;

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