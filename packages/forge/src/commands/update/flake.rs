//! Flake-related utilities for the update command

use anyhow::Result;
use futures::future::join_all;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::commands::executor::run_capture;

/// Maximum number of commits to fetch per input (to avoid huge responses)
const MAX_COMMITS_TO_FETCH: usize = 10;

/// Commit info from GitHub API
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
}

/// Information about a changed flake input
#[derive(Debug, Clone)]
pub struct FlakeInputChange {
    pub name: String,
    pub owner: String,
    pub repo: String,
    pub old_rev: String,
    pub new_rev: String,
    pub new_last_modified: Option<i64>,
    pub commits: Vec<CommitInfo>,
    pub total_commits: usize,
    pub compare_url: Option<String>,
}

/// Flake.lock JSON structure
#[derive(Debug, Deserialize)]
struct FlakeLock {
    nodes: HashMap<String, FlakeNode>,
}

#[derive(Debug, Deserialize)]
struct FlakeNode {
    locked: Option<LockedInfo>,
}

#[derive(Debug, Deserialize)]
struct LockedInfo {
    owner: Option<String>,
    repo: Option<String>,
    rev: Option<String>,
    #[serde(rename = "lastModified")]
    last_modified: Option<i64>,
    #[serde(rename = "type")]
    source_type: Option<String>,
}

/// GitHub API compare response
#[derive(Debug, Deserialize)]
struct GitHubCompareResponse {
    total_commits: usize,
    commits: Vec<GitHubCommit>,
}

#[derive(Debug, Deserialize)]
struct GitHubCommit {
    sha: String,
    commit: GitHubCommitInfo,
}

#[derive(Debug, Deserialize)]
struct GitHubCommitInfo {
    message: String,
}

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

/// Save a copy of flake.lock before updating
pub async fn save_flake_lock_backup(dir: &Path) -> Option<PathBuf> {
    let lock_path = dir.join("flake.lock");
    if !lock_path.exists() {
        return None;
    }

    let backup_path = std::env::temp_dir().join(format!(
        "forge-flake-{}-{}.lock.old",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    ));
    let (success, _, _) = run_capture("cp", &[lock_path.to_str()?, backup_path.to_str()?])
        .await
        .ok()?;

    if success {
        Some(backup_path)
    } else {
        None
    }
}

/// Parse changes in flake.lock between old backup and current
pub async fn parse_flake_changes(dir: &Path, backup_path: &Path) -> Result<Vec<FlakeInputChange>> {
    let lock_path = dir.join("flake.lock");

    if !lock_path.exists() || !backup_path.exists() {
        return Ok(Vec::new());
    }

    // Read both files
    let old_content = tokio::fs::read_to_string(&backup_path).await?;
    let new_content = tokio::fs::read_to_string(&lock_path).await?;

    // Parse JSON
    let old_lock: FlakeLock = serde_json::from_str(&old_content)?;
    let new_lock: FlakeLock = serde_json::from_str(&new_content)?;

    // Find changed inputs
    let mut changes = Vec::new();

    for (name, new_node) in &new_lock.nodes {
        // Skip the root node
        if name == "root" {
            continue;
        }

        let Some(new_locked) = &new_node.locked else {
            continue;
        };

        // Only handle GitHub sources
        if new_locked.source_type.as_deref() != Some("github") {
            continue;
        }

        let Some(new_rev) = &new_locked.rev else {
            continue;
        };
        let Some(owner) = &new_locked.owner else {
            continue;
        };
        let Some(repo) = &new_locked.repo else {
            continue;
        };

        // Check if this input existed before and has changed
        if let Some(old_node) = old_lock.nodes.get(name) {
            if let Some(old_locked) = &old_node.locked {
                if let Some(old_rev) = &old_locked.rev {
                    if old_rev != new_rev {
                        changes.push(FlakeInputChange {
                            name: name.clone(),
                            owner: owner.clone(),
                            repo: repo.clone(),
                            old_rev: old_rev.clone(),
                            new_rev: new_rev.clone(),
                            new_last_modified: new_locked.last_modified,
                            commits: Vec::new(),
                            total_commits: 0,
                            compare_url: Some(format!(
                                "https://github.com/{}/{}/compare/{}...{}",
                                owner,
                                repo,
                                &old_rev[..7.min(old_rev.len())],
                                &new_rev[..7.min(new_rev.len())]
                            )),
                        });
                    }
                }
            }
        }
    }

    // Fetch commit messages from GitHub API
    fetch_commits_for_changes(&mut changes).await;

    // Note: the caller owns the backup file's lifecycle (it may still be needed
    // to revert flake.lock, e.g. on an NVIDIA driver incompatibility) and
    // removes it once the update outcome is settled.
    Ok(changes)
}

/// Fetch commit messages from GitHub API for each changed input
async fn fetch_commits_for_changes(changes: &mut Vec<FlakeInputChange>) {
    let client = match reqwest::Client::builder()
        .user_agent("forge-nixos-tool")
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to create HTTP client: {}", e);
            return;
        }
    };

    let results = join_all(changes.iter().map(|change| async {
        let result = fetch_github_commits(&client, change).await;
        (change.owner.clone(), change.repo.clone(), result)
    }))
    .await;

    for (change, (owner, repo, result)) in changes.iter_mut().zip(results) {
        match result {
            Ok((commits, total)) => {
                change.commits = commits;
                change.total_commits = total;
            }
            Err(e) => {
                tracing::debug!("Failed to fetch commits for {}/{}: {}", owner, repo, e);
            }
        }
    }
}

/// Fetch commits between two revisions from GitHub API (with retry)
async fn fetch_github_commits(
    client: &reqwest::Client,
    change: &FlakeInputChange,
) -> Result<(Vec<CommitInfo>, usize)> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/compare/{}...{}",
        change.owner, change.repo, change.old_rev, change.new_rev
    );

    // Retry the HTTP request with backoff for transient failures
    let max_attempts = 3u32;
    let mut last_error = None;

    for attempt in 0..max_attempts {
        if attempt > 0 {
            let delay = std::time::Duration::from_secs(2u64.saturating_pow(attempt));
            tokio::time::sleep(delay).await;
        }

        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let compare: GitHubCompareResponse = response.json().await?;
                    let commits: Vec<CommitInfo> = compare
                        .commits
                        .iter()
                        .rev()
                        .take(MAX_COMMITS_TO_FETCH)
                        .map(|c| CommitInfo {
                            hash: c.sha[..7.min(c.sha.len())].to_string(),
                            message: c.commit.message.lines().next().unwrap_or("").to_string(),
                        })
                        .collect();
                    return Ok((commits, compare.total_commits));
                } else if response.status().is_server_error() || response.status().as_u16() == 429 {
                    last_error = Some(anyhow::anyhow!("GitHub API returned {}", response.status()));
                    continue;
                } else {
                    anyhow::bail!("GitHub API returned {}", response.status());
                }
            }
            Err(e) => {
                last_error = Some(e.into());
                continue;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries exhausted")))
}
