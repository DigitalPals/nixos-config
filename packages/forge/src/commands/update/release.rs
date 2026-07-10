//! Release-tracked flake input updates.

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::cmp::Ordering;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
struct ReleaseTrackedInput {
    name: &'static str,
    owner: &'static str,
    repo: &'static str,
}

#[derive(Debug, Clone)]
pub struct ReleaseInputUpdate {
    pub name: String,
    pub old_tag: String,
    pub new_tag: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubTag {
    name: String,
}

const RELEASE_TRACKED_INPUTS: &[ReleaseTrackedInput] = &[ReleaseTrackedInput {
    name: "noctalia",
    owner: "noctalia-dev",
    repo: "noctalia",
}];

static NOCTALIA_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)(?P<input>\bnoctalia\s*=\s*\{.*?url\s*=\s*")(?P<url>github:noctalia-dev/noctalia/(?P<tag>v[0-9][^"]*))(?P<suffix>";)"#,
    )
    .expect("valid Noctalia input URL regex")
});

pub async fn update_release_tracked_inputs(
    flake_dir: &Path,
    selected_inputs: &[String],
) -> Result<Vec<ReleaseInputUpdate>> {
    let inputs = RELEASE_TRACKED_INPUTS
        .iter()
        .copied()
        .filter(|input| should_update_input(input.name, selected_inputs))
        .collect::<Vec<_>>();

    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    let flake_path = flake_dir.join("flake.nix");
    let mut content = tokio::fs::read_to_string(&flake_path)
        .await
        .with_context(|| format!("reading {}", flake_path.display()))?;
    let client = reqwest::Client::builder()
        .user_agent("forge-nixos-tool")
        .timeout(Duration::from_secs(15))
        .build()?;

    let mut updates = Vec::new();
    for input in inputs {
        let latest_tag = fetch_latest_release_tag(&client, input.owner, input.repo).await?;
        if let Some(update) = update_input_tag(&mut content, input, &latest_tag)? {
            updates.push(update);
        }
    }

    if !updates.is_empty() {
        tokio::fs::write(&flake_path, content)
            .await
            .with_context(|| format!("writing {}", flake_path.display()))?;
    }

    Ok(updates)
}

fn should_update_input(name: &str, selected_inputs: &[String]) -> bool {
    selected_inputs.is_empty() || selected_inputs.iter().any(|input| input == name)
}

async fn fetch_latest_release_tag(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
) -> Result<String> {
    let release_url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    if let Ok(response) = client.get(&release_url).send().await {
        if response.status().is_success() {
            let release: GitHubRelease = response.json().await?;
            if version_key(&release.tag_name).is_some() {
                return Ok(release.tag_name);
            }
        }
    }

    let tags_url = format!("https://api.github.com/repos/{owner}/{repo}/tags?per_page=100");
    let response = client.get(&tags_url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "GitHub tag lookup for {owner}/{repo} failed with HTTP {}",
            response.status()
        ));
    }

    let tags: Vec<GitHubTag> = response.json().await?;
    tags.into_iter()
        .filter_map(|tag| version_key(&tag.name).map(|key| (tag.name, key)))
        .max_by(|(_, left), (_, right)| compare_version_keys(left, right))
        .map(|(name, _)| name)
        .ok_or_else(|| anyhow!("no semver release tags found for {owner}/{repo}"))
}

fn update_input_tag(
    content: &mut String,
    input: ReleaseTrackedInput,
    latest_tag: &str,
) -> Result<Option<ReleaseInputUpdate>> {
    let captures = match input.name {
        "noctalia" => NOCTALIA_URL_RE
            .captures(content)
            .ok_or_else(|| anyhow!("could not find {} GitHub URL in flake.nix", input.name))?,
        _ => return Err(anyhow!("unsupported release-tracked input {}", input.name)),
    };

    let current_tag = captures
        .name("tag")
        .map(|tag| tag.as_str())
        .ok_or_else(|| anyhow!("could not parse current {} tag", input.name))?;

    if compare_tags(latest_tag, current_tag) != Some(Ordering::Greater) {
        return Ok(None);
    }

    let url_match = captures
        .name("url")
        .ok_or_else(|| anyhow!("could not parse current {} URL", input.name))?;
    let new_url = format!("github:{}/{}/{}", input.owner, input.repo, latest_tag);
    let old_tag = current_tag.to_string();
    content.replace_range(url_match.start()..url_match.end(), &new_url);

    Ok(Some(ReleaseInputUpdate {
        name: input.name.to_string(),
        old_tag,
        new_tag: latest_tag.to_string(),
    }))
}

fn compare_tags(left: &str, right: &str) -> Option<Ordering> {
    let left = version_key(left)?;
    let right = version_key(right)?;
    Some(compare_version_keys(&left, &right))
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct VersionKey {
    parts: Vec<u64>,
    prerelease: Option<(String, u64)>,
}

fn version_key(tag: &str) -> Option<VersionKey> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let (numeric, prerelease) = version
        .split_once('-')
        .map_or((version, None), |(numeric, prerelease)| {
            (numeric, Some(prerelease))
        });
    let parts = numeric
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.is_empty() {
        return None;
    }
    let prerelease = prerelease.map(|value| {
        let digit_index = value.find(|character: char| character.is_ascii_digit());
        match digit_index {
            Some(index) => {
                let (label, number) = value.split_at(index);
                (
                    label.to_ascii_lowercase(),
                    number.parse::<u64>().unwrap_or(0),
                )
            }
            None => (value.to_ascii_lowercase(), 0),
        }
    });

    Some(VersionKey { parts, prerelease })
}

fn compare_version_keys(left: &VersionKey, right: &VersionKey) -> Ordering {
    let max_len = left.parts.len().max(right.parts.len());
    for index in 0..max_len {
        let left_part = left.parts.get(index).copied().unwrap_or(0);
        let right_part = right.parts.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }

    match (&left.prerelease, &right.prerelease) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => left.cmp(right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOCTALIA: ReleaseTrackedInput = ReleaseTrackedInput {
        name: "noctalia",
        owner: "noctalia-dev",
        repo: "noctalia",
    };

    #[test]
    fn updates_noctalia_prerelease_tag_when_latest_is_newer() {
        let mut content = r#"
{
  inputs = {
    noctalia = {
      url = "github:noctalia-dev/noctalia/v5.0.0-beta1";
    };
  };
}
"#
        .to_string();

        let update = update_input_tag(&mut content, NOCTALIA, "v5.0.0-beta2")
            .unwrap()
            .unwrap();

        assert_eq!(update.name, "noctalia");
        assert_eq!(update.old_tag, "v5.0.0-beta1");
        assert_eq!(update.new_tag, "v5.0.0-beta2");
        assert!(content.contains(r#"url = "github:noctalia-dev/noctalia/v5.0.0-beta2";"#));
    }

    #[test]
    fn keeps_noctalia_tag_when_latest_is_not_newer() {
        let mut content = r#"
noctalia = {
  url = "github:noctalia-dev/noctalia/v5.0.0-beta2";
};
"#
        .to_string();

        let update = update_input_tag(&mut content, NOCTALIA, "v5.0.0-beta1").unwrap();

        assert!(update.is_none());
        assert!(content.contains("v5.0.0-beta2"));
    }

    #[test]
    fn compares_multi_digit_versions() {
        assert_eq!(compare_tags("v0.10.0", "v0.9.9"), Some(Ordering::Greater));
        assert_eq!(compare_tags("v1.0.0", "v1.0"), Some(Ordering::Equal));
        assert_eq!(compare_tags("v0.6.0", "v0.6.1"), Some(Ordering::Less));
        assert_eq!(
            compare_tags("v5.0.0-beta2", "v5.0.0-beta1"),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare_tags("v5.0.0", "v5.0.0-beta2"),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn respects_selected_inputs() {
        assert!(should_update_input("noctalia", &[]));
        assert!(should_update_input("noctalia", &["noctalia".to_string()]));
        assert!(!should_update_input("noctalia", &["nixpkgs".to_string()]));
    }
}
