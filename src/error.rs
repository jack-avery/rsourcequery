use thiserror::Error;
use tokio::time::error::Elapsed;

/// Possible errors for the package.
#[derive(Error, Debug)]
pub enum SourceQueryError {
    /// Returned if we received a packet that does not have a header known to us.
    #[error("unknown packet header: {0}")]
    UnknownPacketHeader(i32),
    /// Returned if we received a packet that does not have a type known to us.
    #[error("unknown packet type: {0}")]
    UnknownPacketType(u8),
    /// Returned if the header is mangled in some way (bad offsets, incomplete
    /// response)
    #[error("packet header malformed (can't parse size, id or type)")]
    MalformedPacketHeader(#[from] std::array::TryFromSliceError),
    /// Returned if the body is mangled in some way.
    #[error("packet body malformed (not valid ascii or utf-8)")]
    MalformedPacketBody(#[from] std::str::Utf8Error),
    /// Returned if the host is down or behind a firewall.
    #[error("host cannot be reached")]
    UnreachableHost(#[source] std::io::Error),
    /// The host responded with another challenge after resolving a previous challenge.
    #[error("host ignored to challenge response: {0}")]
    FussyHost(String),
    /// Internal error used if the stream was successfully established, but
    /// there was a problem writing to the socket.
    #[error("cannot send message to host")]
    SendError(#[source] std::io::Error),
    /// Internal error used if the stream was successfully established, but
    /// there was a problem reading from the socket.
    #[error("cannot receive response from host")]
    ReceiveError(#[source] std::io::Error),
    /// Returned if we failed to bind a UDP socket to a port.
    #[error("failed bind port: {0}")]
    FailedPortBind(#[source] std::io::Error),
    /// Returned if the server did not respond in time.
    #[error("timeout")]
    TimeoutError(#[from] Elapsed),
    /// Attempting to parse an empty packet
    #[error("attempt to parse an empty packet")]
    AttemptParseEmptyPacket(),
}
