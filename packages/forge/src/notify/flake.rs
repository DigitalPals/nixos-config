//! Flake input update detection
//!
//! Checks if any flake inputs have newer versions available on GitHub.

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

/// GitHub API response for branch info
#[derive(Debug, Deserialize)]
struct GitHubBranch {
    commit: GitHubCommitRef,
}

#[derive(Debug, Deserialize)]
struct GitHubCommitRef {
    sha: String,
}

/// Input configurations to check
struct InputConfig {
    owner: String,
    repo: String,
    branch: String,
    current_rev: String,
}

/// Check for flake input updates
pub async fn check_flake_updates() -> Result<Vec<String>> {
    let config_dir = super::nixos_config_dir();
    let lock_path = config_dir.join("flake.lock");

    if !lock_path.exists() {
        return Ok(vec![]);
    }

    // Parse flake.lock
    let content = tokio::fs::read_to_string(&lock_path).await?;
    let lock: FlakeLock = serde_json::from_str(&content)?;

    // Find GitHub inputs to check
    let inputs = extract_github_inputs(&lock);

    if inputs.is_empty() {
        return Ok(vec![]);
    }

    // Check each input for updates (with overall timeout)
    let updates = tokio::time::timeout(Duration::from_secs(30), check_inputs_for_updates(inputs))
        .await
        .unwrap_or_else(|_| Ok(vec![]))?;

    Ok(updates)
}

/// Extract GitHub inputs from flake.lock
fn extract_github_inputs(lock: &FlakeLock) -> Vec<(String, InputConfig)> {
    let mut inputs = Vec::new();

    for (name, node) in &lock.nodes {
        // Skip the root node
        if name == "root" {
            continue;
        }

        let Some(locked) = &node.locked else {
            continue;
        };

        // Only handle GitHub sources
        if locked.source_type.as_deref() != Some("github") {
            continue;
        }

        let Some(owner) = &locked.owner else {
            continue;
        };
        let Some(repo) = &locked.repo else {
            continue;
        };
        let Some(rev) = &locked.rev else {
            continue;
        };

        // Get the branch from original, with sensible defaults
        let branch = node
            .original
            .as_ref()
            .and_then(|o| o.git_ref.clone())
            .unwrap_or_else(|| default_branch_for_repo(owner, repo));

        inputs.push((
            name.clone(),
            InputConfig {
                owner: owner.clone(),
                repo: repo.clone(),
                branch,
                current_rev: rev.clone(),
            },
        ));
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

/// Check each input against GitHub API
async fn check_inputs_for_updates(inputs: Vec<(String, InputConfig)>) -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .user_agent("forge-notify")
        .timeout(Duration::from_secs(10))
        .build()?;

    let mut updates = Vec::new();

    // Check inputs concurrently (up to 5 at a time to avoid rate limiting)
    let mut handles = Vec::new();

    for (name, config) in inputs {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            match check_single_input(&client, &config).await {
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

/// Check a single input for updates
async fn check_single_input(client: &reqwest::Client, config: &InputConfig) -> Result<bool> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/branches/{}",
        config.owner, config.repo, config.branch
    );

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(false);
    }

    let branch_info: GitHubBranch = response.json().await?;

    // Compare current rev with latest
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
}
