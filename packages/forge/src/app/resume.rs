//! Operation resume support
//!
//! Detects incomplete operations on startup and offers to resume them.
//! Stores pending operation state to ~/.local/share/forge/pending-operation.json

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A pending (incomplete) operation that can be resumed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOperation {
    /// The operation type (e.g., "update", "install")
    pub operation: String,
    /// When the operation was started (ISO 8601)
    pub started_at: String,
    /// Current step index
    pub current_step: usize,
    /// Total number of steps
    pub total_steps: usize,
}

/// Path to the pending operation file
fn pending_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local/share/forge/pending-operation.json")
}

/// Save a pending operation marker
pub fn save_pending(operation: &str, current_step: usize, total_steps: usize) {
    let pending = PendingOperation {
        operation: operation.to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
        current_step,
        total_steps,
    };

    let path = pending_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&pending) {
        let _ = std::fs::write(&path, json);
    }
}

/// Load a pending operation (if any)
pub fn load_pending() -> Option<PendingOperation> {
    let path = pending_path();
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Clear the pending operation marker (on successful completion)
pub fn clear_pending() {
    let _ = std::fs::remove_file(pending_path());
}
