/// Record and replay game sessions as asciinema v2 compatible files.
///
/// The asciinema v2 format is a newline-delimited JSON (NDJSON) file where
/// the first line is a header object and subsequent lines are event arrays:
///
/// ```text
/// {"version": 2, "width": 80, "height": 24, "timestamp": 1234567890}
/// [0.5, "o", "hello"]
/// [1.0, "i", "q"]
/// ```
///
/// Event types:
/// - `"o"` — output (terminal data sent to the screen)
/// - `"i"` — input  (keystrokes from the player)
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the recording/replay subsystem.
#[derive(Debug)]
pub enum RecordingError {
    /// An I/O error occurred while writing or reading the recording file.
    Io(std::io::Error),
    /// The recording file has an invalid format.
    InvalidFormat(String),
    /// Replay is not yet implemented.
    ReplayNotImplemented,
}

impl fmt::Display for RecordingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordingError::Io(e) => write!(f, "Recording I/O error: {e}"),
            RecordingError::InvalidFormat(msg) => {
                write!(f, "Invalid recording format: {msg}")
            }
            RecordingError::ReplayNotImplemented => {
                write!(f, "Replay is not yet implemented (Phase 5 stub)")
            }
        }
    }
}

impl std::error::Error for RecordingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RecordingError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for RecordingError {
    fn from(e: std::io::Error) -> Self {
        RecordingError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// RecordingEventType
// ---------------------------------------------------------------------------

/// The type of event captured during a recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingEventType {
    /// Terminal output frame (data rendered to the screen).
    Output,
    /// Player input (keystrokes).
    Input,
}

impl RecordingEventType {
    /// Return the asciinema v2 type code for this event type.
    fn as_asciinema_code(self) -> &'static str {
        match self {
            RecordingEventType::Output => "o",
            RecordingEventType::Input => "i",
        }
    }
}

// ---------------------------------------------------------------------------
// RecordingEvent
// ---------------------------------------------------------------------------

/// A single captured event (input or output) with a timestamp.
#[derive(Debug, Clone)]
pub struct RecordingEvent {
    /// Milliseconds since the recording started.
    pub timestamp_ms: u64,
    /// Whether this is an output or input event.
    pub event_type: RecordingEventType,
    /// The captured data (terminal bytes for output, key representation for
    /// input).
    pub data: String,
}

// ---------------------------------------------------------------------------
// GameRecorder
// ---------------------------------------------------------------------------

/// Records a game session to an asciinema v2 compatible file.
pub struct GameRecorder {
    /// Path where the recording will be saved.
    output_path: PathBuf,
    /// The instant when recording started, used to compute relative
    /// timestamps.
    start_time: Instant,
    /// Accumulated events.
    events: Vec<RecordingEvent>,
    /// Terminal width for the asciinema header.
    width: u16,
    /// Terminal height for the asciinema header.
    height: u16,
}

impl GameRecorder {
    /// Create a new recorder that will save to `path`.
    ///
    /// The terminal dimensions default to 80x24 and can be overridden with
    /// [`set_dimensions`](Self::set_dimensions).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            output_path: path.into(),
            start_time: Instant::now(),
            events: Vec::new(),
            width: 80,
            height: 24,
        }
    }

    /// Override the terminal dimensions written to the asciinema header.
    pub fn set_dimensions(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    /// Return the output path.
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    /// Return the number of events recorded so far.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Record a terminal output frame.
    pub fn record_output(&mut self, frame: &str) {
        let elapsed = self.start_time.elapsed();
        self.events.push(RecordingEvent {
            timestamp_ms: elapsed.as_millis() as u64,
            event_type: RecordingEventType::Output,
            data: frame.to_string(),
        });
    }

    /// Record player input.
    pub fn record_input(&mut self, key: &str) {
        let elapsed = self.start_time.elapsed();
        self.events.push(RecordingEvent {
            timestamp_ms: elapsed.as_millis() as u64,
            event_type: RecordingEventType::Input,
            data: key.to_string(),
        });
    }

    /// Write all recorded events to disk in asciinema v2 format (NDJSON).
    ///
    /// The file consists of:
    /// 1. A JSON header line with version, dimensions, and UNIX timestamp.
    /// 2. One JSON array line per event: `[time_seconds, type_code, data]`.
    pub fn save(&self) -> Result<(), RecordingError> {
        let mut file = std::fs::File::create(&self.output_path)?;

        // --- Header line ---
        let unix_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let header = format!(
            r#"{{"version": 2, "width": {}, "height": {}, "timestamp": {}, "title": "NetHack Babel"}}"#,
            self.width, self.height, unix_ts,
        );
        writeln!(file, "{header}")?;

        // --- Event lines ---
        for event in &self.events {
            let seconds = event.timestamp_ms as f64 / 1000.0;
            let type_code = event.event_type.as_asciinema_code();
            // JSON-escape the data string to handle special characters.
            let escaped = json_escape(&event.data);
            writeln!(file, "[{seconds:.3}, \"{type_code}\", \"{escaped}\"]")?;
        }

        file.flush()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Replay stub
// ---------------------------------------------------------------------------

/// Replay a previously recorded session.
///
/// This is a Phase 5 stub — the actual replay implementation will parse the
/// NDJSON file and feed events back through the terminal at the recorded
/// pace.
pub fn replay_session(path: &Path) -> Result<(), RecordingError> {
    if !path.exists() {
        return Err(RecordingError::InvalidFormat(format!(
            "Recording file not found: {}",
            path.display()
        )));
    }
    eprintln!("Replay: would play back {}", path.display());
    Err(RecordingError::ReplayNotImplemented)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal JSON string escaping (enough for asciinema compatibility).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                // Unicode escape for control characters.
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// Helper: create a unique temp file path.
    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("nethack-babel-rec-test");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn recorder_new_defaults() {
        let r = GameRecorder::new("/tmp/test.cast");
        assert_eq!(r.event_count(), 0);
        assert_eq!(r.output_path(), Path::new("/tmp/test.cast"));
    }

    #[test]
    fn record_output_and_input() {
        let mut r = GameRecorder::new("/tmp/test.cast");
        r.record_output("Hello, dungeon!\n");
        r.record_input("h");
        r.record_output("You move west.\n");
        assert_eq!(r.event_count(), 3);
        assert_eq!(r.events[0].event_type, RecordingEventType::Output);
        assert_eq!(r.events[1].event_type, RecordingEventType::Input);
        assert_eq!(r.events[2].event_type, RecordingEventType::Output);
    }

    #[test]
    fn save_produces_valid_ndjson() {
        let path = temp_path("save_test.cast");

        let mut r = GameRecorder::new(&path);
        r.set_dimensions(120, 40);
        // Use a fixed start time to make timestamps predictable.
        r.events.push(RecordingEvent {
            timestamp_ms: 0,
            event_type: RecordingEventType::Output,
            data: "Welcome to NetHack Babel!\n".into(),
        });
        r.events.push(RecordingEvent {
            timestamp_ms: 500,
            event_type: RecordingEventType::Input,
            data: "j".into(),
        });
        r.events.push(RecordingEvent {
            timestamp_ms: 1000,
            event_type: RecordingEventType::Output,
            data: "You move south.\n".into(),
        });

        r.save().unwrap();

        // Read back and verify structure.
        let mut contents = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();

        let lines: Vec<&str> = contents.lines().collect();
        assert!(lines.len() >= 4, "Expected 4+ lines, got {}", lines.len());

        // Header line should contain version 2 and our dimensions.
        assert!(lines[0].contains("\"version\": 2"), "header: {}", lines[0]);
        assert!(lines[0].contains("\"width\": 120"), "header: {}", lines[0]);
        assert!(lines[0].contains("\"height\": 40"), "header: {}", lines[0]);

        // First event: output at time 0.
        assert!(lines[1].starts_with("[0.000"), "event 1: {}", lines[1]);
        assert!(lines[1].contains("\"o\""), "event 1: {}", lines[1]);

        // Second event: input at 0.5s.
        assert!(lines[2].starts_with("[0.500"), "event 2: {}", lines[2]);
        assert!(lines[2].contains("\"i\""), "event 2: {}", lines[2]);

        // Third event: output at 1.0s.
        assert!(lines[3].starts_with("[1.000"), "event 3: {}", lines[3]);
        assert!(lines[3].contains("\"o\""), "event 3: {}", lines[3]);

        // Clean up.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_escape_handles_special_chars() {
        assert_eq!(json_escape("hello"), "hello");
        assert_eq!(json_escape("a\"b"), "a\\\"b");
        assert_eq!(json_escape("a\\b"), "a\\\\b");
        assert_eq!(json_escape("line1\nline2"), "line1\\nline2");
        assert_eq!(json_escape("\t\r"), "\\t\\r");
    }

    #[test]
    fn replay_nonexistent_file_errors() {
        let result = replay_session(Path::new("/nonexistent/file.cast"));
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("not found"), "got: {msg}");
    }

    #[test]
    fn replay_existing_file_returns_not_implemented() {
        let path = temp_path("replay_stub.cast");
        std::fs::write(&path, "{}").unwrap();

        let result = replay_session(&path);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("not yet implemented"), "got: {msg}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_dimensions_works() {
        let mut r = GameRecorder::new("/tmp/test.cast");
        r.set_dimensions(132, 50);
        assert_eq!(r.width, 132);
        assert_eq!(r.height, 50);
    }
}
