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
    name: "lumen",
    owner: "DigitalPals",
    repo: "Lumen",
}];

static LUMEN_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)(?P<input>\blumen\s*=\s*\{.*?url\s*=\s*")(?P<url>github:DigitalPals/Lumen/(?P<tag>v[0-9][^"]*))(?P<suffix>";)"#,
    )
    .expect("valid Lumen input URL regex")
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
        "lumen" => LUMEN_URL_RE
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

fn version_key(tag: &str) -> Option<Vec<u64>> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let parts = version
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.is_empty() {
        return None;
    }
    Some(parts)
}

fn compare_version_keys(left: &[u64], right: &[u64]) -> Ordering {
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let left_part = left.get(index).copied().unwrap_or(0);
        let right_part = right.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUMEN: ReleaseTrackedInput = ReleaseTrackedInput {
        name: "lumen",
        owner: "DigitalPals",
        repo: "Lumen",
    };

    #[test]
    fn updates_lumen_tag_when_latest_is_newer() {
        let mut content = r#"
{
  inputs = {
    lumen = {
      url = "github:DigitalPals/Lumen/v0.6.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
}
"#
        .to_string();

        let update = update_input_tag(&mut content, LUMEN, "v0.6.1")
            .unwrap()
            .unwrap();

        assert_eq!(update.name, "lumen");
        assert_eq!(update.old_tag, "v0.6.0");
        assert_eq!(update.new_tag, "v0.6.1");
        assert!(content.contains(r#"url = "github:DigitalPals/Lumen/v0.6.1";"#));
    }

    #[test]
    fn keeps_lumen_tag_when_latest_is_not_newer() {
        let mut content = r#"
lumen = {
  url = "github:DigitalPals/Lumen/v0.6.1";
};
"#
        .to_string();

        let update = update_input_tag(&mut content, LUMEN, "v0.6.0").unwrap();

        assert!(update.is_none());
        assert!(content.contains("v0.6.1"));
    }

    #[test]
    fn compares_multi_digit_versions() {
        assert_eq!(compare_tags("v0.10.0", "v0.9.9"), Some(Ordering::Greater));
        assert_eq!(compare_tags("v1.0.0", "v1.0"), Some(Ordering::Equal));
        assert_eq!(compare_tags("v0.6.0", "v0.6.1"), Some(Ordering::Less));
    }

    #[test]
    fn respects_selected_inputs() {
        assert!(should_update_input("lumen", &[]));
        assert!(should_update_input("lumen", &["lumen".to_string()]));
        assert!(!should_update_input("lumen", &["nixpkgs".to_string()]));
    }
}
