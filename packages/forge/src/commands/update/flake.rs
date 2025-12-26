//! Flake-related utilities for the update command

use anyhow::Result;
use std::path::Path;

use crate::commands::executor::run_capture;

/// Get the SHA256 hash of flake.lock file
pub async fn get_flake_lock_hash(dir: &Path) -> Option<String> {
    let lock_path = dir.join("flake.lock");
    if lock_path.exists() {
        let (_, stdout, _) = run_capture("sha256sum", &[lock_path.to_str()?])
            .await
            .ok()?;
        Some(stdout.split_whitespace().next()?.to_string())
    } else {
        None
    }
}

/// Parse changes in flake.lock between updates
pub async fn parse_flake_changes(dir: &Path) -> Result<Vec<(String, String, String)>> {
    // TODO: Implement flake.lock change parsing
    // This would require:
    // 1. Saving the old flake.lock before update (e.g., to /tmp/flake.lock.old)
    // 2. Parsing both old and new JSON structures with serde_json
    // 3. Comparing node revisions for each input in the "nodes" object
    // 4. Returning (input_name, old_rev[0..7], new_rev[0..7]) tuples
    //
    // For now, the UI shows "Flake inputs updated:" but no details.
    let lock_path = dir.join("flake.lock");
    if !lock_path.exists() {
        return Ok(Vec::new());
    }

    tracing::debug!("parse_flake_changes: not yet implemented, returning empty");
    Ok(Vec::new())
}
