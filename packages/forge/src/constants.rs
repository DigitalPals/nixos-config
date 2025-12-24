//! Application-wide constants

/// Maximum lines to retain in output buffer
pub const OUTPUT_BUFFER_SIZE: usize = 100;

/// Default command timeout in seconds (5 minutes)
pub const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 300;

/// Event poll timeout in milliseconds
pub const EVENT_POLL_TIMEOUT_MS: u64 = 100;

/// Spinner animation interval in milliseconds
pub const SPINNER_TICK_MS: u128 = 100;

/// Primary user UID (first regular user on NixOS)
pub const PRIMARY_USER_UID: u32 = 1000;

/// Primary user GID (users group on NixOS)
pub const PRIMARY_USER_GID: u32 = 100;

/// Channel buffer size for command messages
pub const COMMAND_CHANNEL_SIZE: usize = 100;

/// Maximum length for user text input (prevents memory exhaustion)
pub const MAX_INPUT_LENGTH: usize = 100;
