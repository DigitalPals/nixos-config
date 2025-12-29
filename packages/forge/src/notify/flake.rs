//! Flake input update detection
//!
//! Checks if any flake inputs have newer versions available on GitHub.
//! To minimize API calls (and avoid rate limiting), only checks nixpkgs
//! by default since that's where most updates come from.

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

/// Flake.lock JSON structure
#[derive(Debug, Deserialize)]
struct FlakeLock {
    nodes: HashMap<String, FlakeNode>,
}

#[derive(Debug, Deserialize)]
struct FlakeNode {
    locked: Option<LockedInfo>,
    original: Option<OriginalInfo>,
}

#[derive(Debug, Deserialize)]
struct LockedInfo {
    owner: Option<String>,
    repo: Option<String>,
    rev: Option<String>,
    #[serde(rename = "type")]
    source_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OriginalInfo {
    #[serde(rename = "ref")]
    git_ref: Option<String>,
}

/// Input configurations to check
struct InputConfig {
    owner: String,
    repo: String,
    branch: String,
    current_rev: String,
}

/// Priority inputs to check (in order of importance)
/// We only check these to minimize API calls
const PRIORITY_INPUTS: &[&str] = &["nixpkgs"];

/// Check for flake input updates (only checks priority inputs to save API calls)
pub async fn check_flake_updates() -> Result<Vec<String>> {
    let config_dir = super::nixos_config_dir();
    let lock_path = config_dir.join("flake.lock");

    if !lock_path.exists() {
        return Ok(vec![]);
    }

    // Parse flake.lock
    let content = tokio::fs::read_to_string(&lock_path).await?;
    let lock: FlakeLock = serde_json::from_str(&content)?;

    // Find GitHub inputs to check (only priority inputs)
    let inputs = extract_priority_inputs(&lock);

    if inputs.is_empty() {
        return Ok(vec![]);
    }

    // Check inputs with timeout (typically just 1 API call for nixpkgs)
    let updates = tokio::time::timeout(
        Duration::from_secs(15),
        check_inputs_rest(inputs),
    )
    .await
    .unwrap_or_else(|_| Ok(vec![]))?;

    Ok(updates)
}

/// Extract only priority GitHub inputs from flake.lock (to minimize API calls)
fn extract_priority_inputs(lock: &FlakeLock) -> Vec<(String, InputConfig)> {
    let mut inputs = Vec::new();

    for priority_name in PRIORITY_INPUTS {
        if let Some(node) = lock.nodes.get(*priority_name) {
            if let Some(config) = extract_input_config(*priority_name, node) {
                inputs.push((priority_name.to_string(), config));
            }
        }
    }

    inputs
}

/// Extract config for a single input node
fn extract_input_config(_name: &str, node: &FlakeNode) -> Option<InputConfig> {
    let locked = node.locked.as_ref()?;

    // Only handle GitHub sources
    if locked.source_type.as_deref() != Some("github") {
        return None;
    }

    let owner = locked.owner.as_ref()?;
    let repo = locked.repo.as_ref()?;
    let rev = locked.rev.as_ref()?;

    // Get the branch from original, with sensible defaults
    let branch = node
        .original
        .as_ref()
        .and_then(|o| o.git_ref.clone())
        .unwrap_or_else(|| default_branch_for_repo(owner, repo));

    Some(InputConfig {
        owner: owner.clone(),
        repo: repo.clone(),
        branch,
        current_rev: rev.clone(),
    })
}

/// Extract all GitHub inputs from flake.lock (for potential future use)
#[allow(dead_code)]
fn extract_all_github_inputs(lock: &FlakeLock) -> Vec<(String, InputConfig)> {
    let mut inputs = Vec::new();

    for (name, node) in &lock.nodes {
        if name == "root" {
            continue;
        }
        if let Some(config) = extract_input_config(name, node) {
            inputs.push((name.clone(), config));
        }
    }

    inputs
}

/// Get default branch for well-known repos
fn default_branch_for_repo(owner: &str, repo: &str) -> String {
    match (owner, repo) {
        ("NixOS", "nixpkgs") => "nixos-unstable".to_string(),
        ("nix-community", "home-manager") => "master".to_string(),
        _ => "main".to_string(),
    }
}

/// Check inputs using REST API
async fn check_inputs_rest(inputs: Vec<(String, InputConfig)>) -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .user_agent("forge-notify")
        .timeout(Duration::from_secs(10))
        .build()?;

    let mut updates = Vec::new();
    let mut handles = Vec::new();

    for (name, config) in inputs {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            match check_single_input_rest(&client, &config).await {
                Ok(true) => Some(name),
                _ => None,
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        if let Ok(Some(name)) = handle.await {
            updates.push(name);
        }
    }

    Ok(updates)
}

/// Check a single input using REST API
async fn check_single_input_rest(client: &reqwest::Client, config: &InputConfig) -> Result<bool> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/branches/{}",
        config.owner, config.repo, config.branch
    );

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(false);
    }

    #[derive(Deserialize)]
    struct BranchResponse {
        commit: CommitRef,
    }

    #[derive(Deserialize)]
    struct CommitRef {
        sha: String,
    }

    let branch_info: BranchResponse = response.json().await?;
    Ok(branch_info.commit.sha != config.current_rev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_branch_nixpkgs() {
        assert_eq!(
            default_branch_for_repo("NixOS", "nixpkgs"),
            "nixos-unstable"
        );
    }

    #[test]
    fn test_default_branch_home_manager() {
        assert_eq!(
            default_branch_for_repo("nix-community", "home-manager"),
            "master"
        );
    }

    #[test]
    fn test_default_branch_other() {
        assert_eq!(default_branch_for_repo("someone", "something"), "main");
    }

    #[test]
    fn test_priority_inputs_contains_nixpkgs() {
        assert!(PRIORITY_INPUTS.contains(&"nixpkgs"));
    }
}
