/// SSH/telnet game server for NetHack Babel.
///
/// Allows multiple players to connect and play remotely. This is a Phase 5
/// stub — no actual networking is implemented yet.
use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that the game server can produce.
#[derive(Debug)]
pub enum ServerError {
    /// The server subsystem has not been implemented yet.
    NotImplemented,
    /// Failed to bind to the requested address.
    BindFailed(String),
    /// The connection limit has been exceeded.
    ConnectionLimitExceeded { max: usize },
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::NotImplemented => {
                write!(f, "Server mode is not yet implemented (Phase 5 stub)")
            }
            ServerError::BindFailed(addr) => {
                write!(f, "Failed to bind to {addr}")
            }
            ServerError::ConnectionLimitExceeded { max } => {
                write!(f, "Connection limit exceeded (max {max})")
            }
        }
    }
}

impl std::error::Error for ServerError {}

// ---------------------------------------------------------------------------
// GameServer
// ---------------------------------------------------------------------------

/// SSH/telnet game server for NetHack Babel.
///
/// Allows multiple players to connect and play remotely.
pub struct GameServer {
    bind_addr: String,
    max_connections: usize,
}

impl GameServer {
    /// Create a new game server that will listen on `bind_addr` and accept up
    /// to `max_connections` simultaneous players.
    pub fn new(bind_addr: &str, max_connections: usize) -> Self {
        Self {
            bind_addr: bind_addr.to_string(),
            max_connections,
        }
    }

    /// Return the address this server is configured to bind to.
    pub fn bind_addr(&self) -> &str {
        &self.bind_addr
    }

    /// Return the maximum number of simultaneous connections.
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }

    /// Start listening for connections.
    ///
    /// This is a placeholder — the actual SSH/telnet server will be
    /// implemented in Phase 5. For now it logs a message and returns an
    /// informative error.
    pub fn start(&self) -> Result<(), ServerError> {
        eprintln!(
            "Server mode: would listen on {} (max {} connections)",
            self.bind_addr, self.max_connections
        );
        eprintln!("Server mode is not yet implemented.");
        Err(ServerError::NotImplemented)
    }
}

/// Default bind address used when the user passes `--server` without an
/// explicit address.
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:2323";

/// Default maximum number of simultaneous connections.
pub const DEFAULT_MAX_CONNECTIONS: usize = 64;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_new_stores_params() {
        let srv = GameServer::new("127.0.0.1:4000", 32);
        assert_eq!(srv.bind_addr(), "127.0.0.1:4000");
        assert_eq!(srv.max_connections(), 32);
    }

    #[test]
    fn server_start_returns_not_implemented() {
        let srv = GameServer::new(DEFAULT_BIND_ADDR, DEFAULT_MAX_CONNECTIONS);
        let err = srv.start().unwrap_err();
        assert!(
            matches!(err, ServerError::NotImplemented),
            "Expected NotImplemented, got: {err}"
        );
    }

    #[test]
    fn error_display() {
        let e = ServerError::NotImplemented;
        let msg = format!("{e}");
        assert!(msg.contains("not yet implemented"), "got: {msg}");

        let e = ServerError::BindFailed("1.2.3.4:5555".into());
        let msg = format!("{e}");
        assert!(msg.contains("1.2.3.4:5555"), "got: {msg}");

        let e = ServerError::ConnectionLimitExceeded { max: 10 };
        let msg = format!("{e}");
        assert!(msg.contains("10"), "got: {msg}");
    }
}
