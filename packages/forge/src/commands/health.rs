//! Pre-operation health checks to verify required tools exist

use crate::commands::executor::command_exists;

/// Operations that can be health-checked
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Operation {
    Install,
    Update,
    AppBackup,
    AppRestore,
    KeySetup,
}

/// A required tool with installation hints
#[derive(Debug, Clone)]
pub struct ToolRequirement {
    pub name: &'static str,
    pub install_hint: &'static str,
    pub required: bool,
}

/// Result of a health check
#[derive(Debug, Default)]
pub struct HealthCheckResult {
    pub missing_required: Vec<ToolRequirement>,
    pub missing_optional: Vec<ToolRequirement>,
}

impl HealthCheckResult {
    pub fn is_ok(&self) -> bool {
        self.missing_required.is_empty()
    }

    /// Format a user-friendly error message
    pub fn error_message(&self) -> String {
        let mut lines = Vec::new();
        if !self.missing_required.is_empty() {
            lines.push("Missing required tools:".to_string());
            for tool in &self.missing_required {
                lines.push(format!("  - {} ({})", tool.name, tool.install_hint));
            }
        }
        if !self.missing_optional.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push("Missing optional tools:".to_string());
            for tool in &self.missing_optional {
                lines.push(format!("  - {} ({})", tool.name, tool.install_hint));
            }
        }
        lines.join("\n")
    }
}

/// Get required tools for an operation
fn tools_for_operation(operation: Operation) -> Vec<ToolRequirement> {
    match operation {
        Operation::Install => vec![
            ToolRequirement {
                name: "nix",
                install_hint: "should be available on NixOS",
                required: true,
            },
            ToolRequirement {
                name: "git",
                install_hint: "add git to system packages",
                required: true,
            },
            ToolRequirement {
                name: "sudo",
                install_hint: "should be available on NixOS",
                required: true,
            },
        ],
        Operation::Update => vec![
            ToolRequirement {
                name: "nix",
                install_hint: "should be available on NixOS",
                required: true,
            },
            ToolRequirement {
                name: "git",
                install_hint: "add git to system packages",
                required: true,
            },
            ToolRequirement {
                name: "sudo",
                install_hint: "should be available on NixOS",
                required: true,
            },
            ToolRequirement {
                name: "nvd",
                install_hint: "add nvd to system packages",
                required: false,
            },
            ToolRequirement {
                name: "fwupdmgr",
                install_hint: "enable services.fwupd",
                required: false,
            },
        ],
        Operation::AppBackup => vec![
            ToolRequirement {
                name: "app-backup",
                install_hint: "enable programs.app-backup and rebuild",
                required: true,
            },
            ToolRequirement {
                name: "git",
                install_hint: "add git to system packages",
                required: true,
            },
        ],
        Operation::AppRestore => vec![
            ToolRequirement {
                name: "app-restore",
                install_hint: "enable programs.app-backup and rebuild",
                required: true,
            },
            ToolRequirement {
                name: "git",
                install_hint: "add git to system packages",
                required: true,
            },
        ],
        Operation::KeySetup => vec![
            ToolRequirement {
                name: "op",
                install_hint: "enable programs._1password and rebuild",
                required: true,
            },
            ToolRequirement {
                name: "age",
                install_hint: "add age to system packages",
                required: true,
            },
            ToolRequirement {
                name: "ssh-keygen",
                install_hint: "add openssh to system packages",
                required: true,
            },
        ],
    }
}

/// Check if all required tools for an operation are available
pub async fn check_health(operation: Operation) -> HealthCheckResult {
    let tools = tools_for_operation(operation);
    let mut result = HealthCheckResult::default();

    for tool in tools {
        if !command_exists(tool.name).await {
            if tool.required {
                result.missing_required.push(tool);
            } else {
                result.missing_optional.push(tool);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_for_operations() {
        // Each operation should have at least one required tool
        for op in [
            Operation::Install,
            Operation::Update,
            Operation::AppBackup,
            Operation::AppRestore,
            Operation::KeySetup,
        ] {
            let tools = tools_for_operation(op);
            assert!(!tools.is_empty());
            assert!(tools.iter().any(|t| t.required));
        }
    }

    #[test]
    fn test_health_check_result_is_ok() {
        let result = HealthCheckResult::default();
        assert!(result.is_ok());

        let result = HealthCheckResult {
            missing_required: vec![ToolRequirement {
                name: "nix",
                install_hint: "",
                required: true,
            }],
            missing_optional: vec![],
        };
        assert!(!result.is_ok());
    }

    #[test]
    fn test_error_message_format() {
        let result = HealthCheckResult {
            missing_required: vec![ToolRequirement {
                name: "nix",
                install_hint: "install nix",
                required: true,
            }],
            missing_optional: vec![ToolRequirement {
                name: "nvd",
                install_hint: "add nvd",
                required: false,
            }],
        };
        let msg = result.error_message();
        assert!(msg.contains("nix"));
        assert!(msg.contains("nvd"));
    }
}
