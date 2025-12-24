//! Disk enumeration utilities

use anyhow::Result;
use serde::Deserialize;
use std::process::Command;

/// Information about a disk device
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskInfo {
    pub path: String,
    pub size: String,
    pub size_bytes: u64,
    pub model: Option<String>,
}

/// JSON structure for lsblk output
#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<BlockDevice>,
}

#[derive(Debug, Deserialize)]
struct BlockDevice {
    name: String,
    size: Option<String>,
    model: Option<String>,
    #[serde(rename = "type")]
    device_type: Option<String>,
}

/// Get list of available disks (excluding loop, ram, rom, zram devices)
pub fn get_available_disks() -> Result<Vec<DiskInfo>> {
    // Use JSON output for reliable parsing (handles model names with spaces)
    let output = Command::new("lsblk")
        .args(["-Jd", "-o", "NAME,SIZE,MODEL,TYPE"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut disks = Vec::new();

    // Parse JSON output
    let lsblk: LsblkOutput = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(_) => {
            // Fallback to text parsing if JSON fails
            return get_available_disks_text_fallback();
        }
    };

    for device in lsblk.blockdevices {
        // Check if it's a disk
        if device.device_type.as_deref() != Some("disk") {
            continue;
        }

        let name = &device.name;

        // Skip non-physical devices
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("zram")
            || name.starts_with("sr")
            || name.starts_with("fd")
        {
            continue;
        }

        let path = format!("/dev/{}", name);
        let size = device.size.clone().unwrap_or_default();
        let size_bytes = parse_size(&size);

        // Clean up model name (remove extra whitespace)
        let model = device.model.as_ref().map(|m| m.trim().to_string()).filter(|m| !m.is_empty());

        disks.push(DiskInfo {
            path,
            size,
            size_bytes,
            model,
        });
    }

    // Sort by size (largest first)
    disks.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    Ok(disks)
}

/// Fallback text-based parsing for older lsblk versions without JSON support
fn get_available_disks_text_fallback() -> Result<Vec<DiskInfo>> {
    let output = Command::new("lsblk")
        .args(["-dno", "NAME,SIZE,TYPE"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut disks = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let name = parts[0];
        let size = parts[1];
        let disk_type = parts[2];

        if disk_type != "disk" {
            continue;
        }

        // Skip non-physical devices
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("zram")
            || name.starts_with("sr")
            || name.starts_with("fd")
        {
            continue;
        }

        let path = format!("/dev/{}", name);
        let size_bytes = parse_size(size);

        disks.push(DiskInfo {
            path,
            size: size.to_string(),
            size_bytes,
            model: None, // Can't reliably parse model in text mode
        });
    }

    // Sort by size (largest first)
    disks.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    Ok(disks)
}

/// Parse size string like "1.8T" to bytes
fn parse_size(size: &str) -> u64 {
    let size = size.trim();
    if size.is_empty() {
        return 0;
    }

    let (num_str, unit) = if size.ends_with('T') {
        (&size[..size.len() - 1], 1024u64 * 1024 * 1024 * 1024)
    } else if size.ends_with('G') {
        (&size[..size.len() - 1], 1024u64 * 1024 * 1024)
    } else if size.ends_with('M') {
        (&size[..size.len() - 1], 1024u64 * 1024)
    } else if size.ends_with('K') {
        (&size[..size.len() - 1], 1024u64)
    } else {
        (size, 1u64)
    };

    match num_str.parse::<f64>() {
        Ok(num) => (num * unit as f64) as u64,
        Err(e) => {
            tracing::warn!("Failed to parse disk size '{}': {}", size, e);
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1T"), 1024 * 1024 * 1024 * 1024);
        // 1.8T should be approximately 1.8 TB
        assert!(parse_size("1.8T") > 1024 * 1024 * 1024 * 1024);
        assert!(parse_size("1.8T") < 2 * 1024 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("500G"), 500 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("256M"), 256 * 1024 * 1024);
    }
}
