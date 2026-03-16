/// SSH/telnet game server for NetHack Babel.
///
/// Allows multiple players to connect and interact over a simple line-based
/// protocol.
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that the game server can produce.
#[derive(Debug)]
pub enum ServerError {
    /// Failed to bind to the requested address.
    BindFailed(String),
    /// Listener setup or accept loop failed.
    Io(String),
    /// The connection limit has been exceeded.
    ConnectionLimitExceeded { max: usize },
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::BindFailed(addr) => {
                write!(f, "Failed to bind to {addr}")
            }
            ServerError::Io(msg) => write!(f, "Server I/O error: {msg}"),
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
    pub fn start(&self) -> Result<(), ServerError> {
        self.run(Arc::new(AtomicBool::new(false)))
    }

    fn run(&self, shutdown: Arc<AtomicBool>) -> Result<(), ServerError> {
        let listener = TcpListener::bind(&self.bind_addr)
            .map_err(|_| ServerError::BindFailed(self.bind_addr.clone()))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| ServerError::Io(format!("set_nonblocking failed: {e}")))?;

        let active_connections = Arc::new(AtomicUsize::new(0));

        eprintln!(
            "Server listening on {} (max {} connections)",
            self.bind_addr, self.max_connections
        );

        while !shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    let active = Arc::clone(&active_connections);
                    let max = self.max_connections;
                    std::thread::spawn(move || {
                        if !acquire_connection_slot(&active, max) {
                            let mut stream = stream;
                            let _ = writeln!(
                                stream,
                                "Server is full (max {max} connections). Try again later."
                            );
                            let _ = stream.flush();
                            return;
                        }

                        let _slot = ConnectionSlot::new(Arc::clone(&active));
                        let mut stream = stream;
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(3600)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
                        let _ = writeln!(stream, "Welcome to NetHack Babel server (MVP).");
                        let _ = writeln!(stream, "Commands: help, ping, status, echo <text>, quit");
                        let _ = write!(stream, "> ");
                        let _ = stream.flush();

                        let mut reader = match stream.try_clone() {
                            Ok(clone) => BufReader::new(clone),
                            Err(_) => return,
                        };

                        loop {
                            let mut line = String::new();
                            match reader.read_line(&mut line) {
                                Ok(0) => break,
                                Ok(_) => match parse_command(&line) {
                                    ClientCommand::Help => {
                                        let _ = writeln!(
                                            stream,
                                            "help - show this message\nping - health check\nstatus - show connection status\necho <text> - echo input\nquit - disconnect"
                                        );
                                    }
                                    ClientCommand::Ping => {
                                        let _ = writeln!(stream, "pong");
                                    }
                                    ClientCommand::Status => {
                                        let current = active.load(Ordering::Relaxed);
                                        let _ = writeln!(
                                            stream,
                                            "active_connections={current} max_connections={max}"
                                        );
                                    }
                                    ClientCommand::Echo(text) => {
                                        let _ = writeln!(stream, "{text}");
                                    }
                                    ClientCommand::Quit => {
                                        let _ = writeln!(stream, "Goodbye.");
                                        let _ = stream.flush();
                                        break;
                                    }
                                    ClientCommand::Unknown => {
                                        let _ = writeln!(stream, "Unknown command. Type 'help'.");
                                    }
                                },
                                Err(e)
                                    if matches!(
                                        e.kind(),
                                        std::io::ErrorKind::WouldBlock
                                            | std::io::ErrorKind::TimedOut
                                    ) =>
                                {
                                    let _ = writeln!(stream, "Timed out waiting for input.");
                                    let _ = stream.flush();
                                    break;
                                }
                                Err(_) => break,
                            }
                            let _ = write!(stream, "> ");
                            let _ = stream.flush();
                        }
                        eprintln!("Connection closed: {peer}");
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(ServerError::Io(format!("accept failed: {e}"))),
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClientCommand {
    Help,
    Ping,
    Status,
    Echo(String),
    Quit,
    Unknown,
}

fn parse_command(line: &str) -> ClientCommand {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return ClientCommand::Unknown;
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or_default().to_ascii_lowercase();
    match cmd.as_str() {
        "help" => ClientCommand::Help,
        "ping" => ClientCommand::Ping,
        "status" => ClientCommand::Status,
        "quit" | "exit" => ClientCommand::Quit,
        "echo" => ClientCommand::Echo(parts.next().unwrap_or_default().trim().to_string()),
        _ => ClientCommand::Unknown,
    }
}

fn acquire_connection_slot(counter: &AtomicUsize, max: usize) -> bool {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        if current >= max {
            return false;
        }
        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}

struct ConnectionSlot {
    counter: Arc<AtomicUsize>,
}

impl ConnectionSlot {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        Self { counter }
    }
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
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
    use std::sync::atomic::AtomicBool;

    #[test]
    fn server_new_stores_params() {
        let srv = GameServer::new("127.0.0.1:4000", 32);
        assert_eq!(srv.bind_addr(), "127.0.0.1:4000");
        assert_eq!(srv.max_connections(), 32);
    }

    #[test]
    fn server_run_returns_bind_failed_for_invalid_addr() {
        let srv = GameServer::new("bad-addr", DEFAULT_MAX_CONNECTIONS);
        let err = srv
            .run(Arc::new(AtomicBool::new(true)))
            .expect_err("invalid address should fail");
        assert!(matches!(err, ServerError::BindFailed(_)));
    }

    #[test]
    fn acquire_slot_respects_limit() {
        let counter = AtomicUsize::new(0);
        assert!(acquire_connection_slot(&counter, 1));
        assert!(!acquire_connection_slot(&counter, 1));
    }

    #[test]
    fn parse_command_variants() {
        assert_eq!(parse_command("help"), ClientCommand::Help);
        assert_eq!(parse_command("PING"), ClientCommand::Ping);
        assert_eq!(parse_command("status"), ClientCommand::Status);
        assert_eq!(
            parse_command("echo hello world"),
            ClientCommand::Echo("hello world".to_string())
        );
        assert_eq!(parse_command("quit"), ClientCommand::Quit);
        assert_eq!(parse_command("unknown"), ClientCommand::Unknown);
    }

    #[test]
    fn error_display() {
        let e = ServerError::BindFailed("1.2.3.4:5555".into());
        let msg = format!("{e}");
        assert!(msg.contains("1.2.3.4:5555"), "got: {msg}");

        let e = ServerError::ConnectionLimitExceeded { max: 10 };
        let msg = format!("{e}");
        assert!(msg.contains("10"), "got: {msg}");
    }
}
