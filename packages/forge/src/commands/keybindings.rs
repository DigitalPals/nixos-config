//! Keybindings parser and editor launcher

use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::app::Keybinding;
use crate::constants::nixos_config_dir;

/// Get the path to the bindings.nix file
pub fn bindings_file_path() -> PathBuf {
    nixos_config_dir().join("home/hyprland/bindings.nix")
}

/// Generate a human-readable description from action and argument
pub fn generate_description(action: &str, argument: &str) -> String {
    // Strip bind type annotations (repeat), (locked), (mouse), (desc)
    let base_action = action
        .trim_end_matches(" (repeat)")
        .trim_end_matches(" (locked)")
        .trim_end_matches(" (mouse)")
        .trim_end_matches(" (desc)");
    let arg_lower = argument.to_lowercase();

    match base_action {
        "exec" => {
            // Terminal
            if arg_lower.contains("ghostty") || argument.contains("$terminal") {
                if arg_lower.contains("lazydocker") {
                    return "Open Lazydocker".to_string();
                }
                if arg_lower.contains("btop") {
                    return "Open System Monitor".to_string();
                }
                return "Open Terminal".to_string();
            }

            // Browser apps (--app=)
            if arg_lower.contains("--app=") {
                if arg_lower.contains("whatsapp") {
                    return "Open WhatsApp".to_string();
                }
                if arg_lower.contains("youtube") {
                    return "Open YouTube".to_string();
                }
                if arg_lower.contains("chatgpt") {
                    return "Open ChatGPT".to_string();
                }
                if arg_lower.contains("photos.google") {
                    return "Open Google Photos".to_string();
                }
                if arg_lower.contains("x.com") {
                    return "Open X (Twitter)".to_string();
                }
                return "Open Web App".to_string();
            }

            // Browser
            if arg_lower.contains("chrome")
                || arg_lower.contains("firefox")
                || argument.contains("$browser")
            {
                if arg_lower.contains("--incognito") || arg_lower.contains("--private") {
                    return "Open Incognito Browser".to_string();
                }
                return "Open Browser".to_string();
            }

            // File manager
            if arg_lower.contains("nautilus")
                || arg_lower.contains("dolphin")
                || arg_lower.contains("thunar")
            {
                return "Open Files".to_string();
            }

            // Common apps
            if arg_lower.contains("spotify") {
                return "Open Spotify".to_string();
            }
            if arg_lower.contains("1password") {
                return "Open 1Password".to_string();
            }
            if arg_lower.contains("gnome-calculator") || arg_lower.contains("calculator") {
                return "Open Calculator".to_string();
            }

            // Screenshot
            if arg_lower.contains("screenshot") {
                if arg_lower.contains("region") {
                    return "Screenshot Region".to_string();
                }
                if arg_lower.contains("fullscreen") {
                    return "Screenshot Fullscreen".to_string();
                }
                return "Take Screenshot".to_string();
            }

            // Volume
            if arg_lower.contains("wpctl") && arg_lower.contains("volume") {
                if arg_lower.contains("%+") || arg_lower.contains("+ ") {
                    return "Volume Up".to_string();
                }
                if arg_lower.contains("%-") || arg_lower.contains("- ") {
                    return "Volume Down".to_string();
                }
            }

            // Mute
            if arg_lower.contains("wpctl") && arg_lower.contains("mute") {
                if arg_lower.contains("source") {
                    return "Toggle Mic Mute".to_string();
                }
                return "Toggle Mute".to_string();
            }

            // Brightness
            if arg_lower.contains("brightnessctl") {
                if arg_lower.contains("%+") || arg_lower.contains("+ ") {
                    return "Brightness Up".to_string();
                }
                if arg_lower.contains("%-") || arg_lower.contains("- ") {
                    return "Brightness Down".to_string();
                }
            }

            // Media player
            if arg_lower.contains("playerctl") {
                if arg_lower.contains("play-pause") {
                    return "Play/Pause Media".to_string();
                }
                if arg_lower.contains("next") {
                    return "Next Track".to_string();
                }
                if arg_lower.contains("previous") || arg_lower.contains("prev") {
                    return "Previous Track".to_string();
                }
            }

            // Lock screen
            if arg_lower.contains("hyprlock") || arg_lower.contains("lockscreen") {
                return "Lock Screen".to_string();
            }

            // Clipboard image
            if arg_lower.contains("clipboard-image") {
                return "Paste Image from Clipboard".to_string();
            }

            // Window opacity toggle
            if arg_lower.contains("alpha") && arg_lower.contains("toggle") {
                return "Toggle Window Opacity".to_string();
            }

            // Launcher
            if arg_lower.contains("launcher")
                || arg_lower.contains("fuzzel")
                || arg_lower.contains("rofi")
            {
                return "Open Launcher".to_string();
            }

            // Fallback for exec
            format!("Run: {}", truncate_arg(argument, 30))
        }

        // Window actions
        "killactive" => "Close Window".to_string(),
        "togglefloating" => "Toggle Floating".to_string(),
        "pseudo" => "Toggle Pseudo Tile".to_string(),
        "togglesplit" => "Toggle Split".to_string(),
        "exit" => "Exit Hyprland".to_string(),

        // Focus navigation
        "movefocus" => match argument.to_lowercase().as_str() {
            "l" => "Focus Left".to_string(),
            "r" => "Focus Right".to_string(),
            "u" => "Focus Up".to_string(),
            "d" => "Focus Down".to_string(),
            _ => format!("Focus {}", argument),
        },

        // Workspaces
        "workspace" => {
            if argument == "e+1" {
                return "Next Workspace".to_string();
            }
            if argument == "e-1" {
                return "Previous Workspace".to_string();
            }
            if let Ok(n) = argument.parse::<u32>() {
                return format!("Switch to Workspace {}", n);
            }
            format!("Switch to Workspace {}", argument)
        }

        "movetoworkspace" => {
            if let Ok(n) = argument.parse::<u32>() {
                return format!("Move to Workspace {}", n);
            }
            format!("Move to Workspace {}", argument)
        }

        // Mouse bindings
        "movewindow" => "Move Window (drag)".to_string(),
        "resizewindow" => "Resize Window (drag)".to_string(),

        // Shortcuts
        "sendshortcut" => {
            let parts: Vec<&str> = argument.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 2 {
                let mods = parts[0].to_uppercase();
                let key = parts[1];
                // Recognize common shortcuts
                if mods == "CTRL" && key == "Insert" {
                    return "Copy".to_string();
                }
                if mods == "SHIFT" && key == "Insert" {
                    return "Paste".to_string();
                }
                if mods == "CTRL" && key.to_uppercase() == "X" {
                    return "Cut".to_string();
                }
                return format!("Send {}+{}", mods, key);
            }
            "Send Shortcut".to_string()
        }

        // Global shortcuts
        "global" => {
            if arg_lower.contains("wallpaper") {
                return "Toggle Wallpaper Selector".to_string();
            }
            format!("Global: {}", truncate_arg(argument, 25))
        }

        // Fallback
        _ => {
            if argument.is_empty() {
                capitalize_action(base_action)
            } else {
                format!(
                    "{}: {}",
                    capitalize_action(base_action),
                    truncate_arg(argument, 25)
                )
            }
        }
    }
}

/// Truncate argument for display
fn truncate_arg(arg: &str, max_len: usize) -> String {
    if arg.len() > max_len {
        format!("{}...", &arg[..max_len - 3])
    } else {
        arg.to_string()
    }
}

/// Capitalize action name for display
fn capitalize_action(action: &str) -> String {
    let mut chars = action.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Detect the current shell from the runtime file
pub fn detect_current_shell() -> String {
    // Try to get UID from environment or /proc
    let uid = std::env::var("UID")
        .ok()
        .or_else(|| {
            // Try to read from /proc/self/status
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|content| {
                    content
                        .lines()
                        .find(|line| line.starts_with("Uid:"))
                        .and_then(|line| line.split_whitespace().nth(1))
                        .map(|s| s.to_string())
                })
        })
        .unwrap_or_else(|| "1000".to_string()); // Default to 1000 if all else fails

    let runtime_file = format!("/run/user/{}/desktop-shell", uid);
    if let Ok(shell) = std::fs::read_to_string(&runtime_file) {
        return shell.trim().to_string();
    }

    "lumen".to_string()
}

/// Parse the bindings.nix file and extract keybindings
pub fn parse_bindings() -> Result<(Vec<Keybinding>, Vec<String>, String)> {
    let bindings_path = bindings_file_path();
    let content = std::fs::read_to_string(&bindings_path)?;
    let shell = detect_current_shell();

    let mut bindings = Vec::new();
    let mut categories = Vec::new();
    let mut current_category = "General".to_string();

    // Track variables for substitution
    let mut variables: HashMap<String, String> = HashMap::new();

    // Track whether we're inside the Hyprland config block (after '')
    let mut in_config_block = false;

    // Regex patterns
    let var_re = Regex::new(r#"^\s*\$(\w+)\s*=\s*(.+)$"#)?;
    let bind_re = Regex::new(r#"^\s*(bind[eldm]*)\s*=\s*(.+)$"#)?;
    let comment_re = Regex::new(r#"^\s*#\s*(.+)$"#)?;

    for line in content.lines() {
        let line = line.trim();

        // Track when we enter the Hyprland config block
        if line == "''" && !in_config_block {
            in_config_block = true;
            continue;
        }

        // Skip empty lines and Nix interpolation
        if line.is_empty()
            || line.starts_with("${")
            || line.starts_with("let")
            || line.starts_with("in")
            || line == "''"
        {
            continue;
        }

        // Check for category comment (only inside the config block)
        if let Some(caps) = comment_re.captures(line) {
            let comment = caps.get(1).map_or("", |m| m.as_str()).trim();
            // Skip inline comments, short comments, comments outside config block, and Variables
            if in_config_block
                && !comment.is_empty()
                && comment.len() > 3
                && !comment.starts_with("Wallpaper")
                && comment != "Variables"
            {
                current_category = comment.to_string();
                if !categories.contains(&current_category) {
                    categories.push(current_category.clone());
                }
            }
            continue;
        }

        // Check for variable definition
        if let Some(caps) = var_re.captures(line) {
            let var_name = caps.get(1).map_or("", |m| m.as_str());
            let var_value = caps.get(2).map_or("", |m| m.as_str()).trim();
            variables.insert(format!("${}", var_name), var_value.to_string());
            continue;
        }

        // Check for bind statement
        if let Some(caps) = bind_re.captures(line) {
            let bind_type = caps.get(1).map_or("bind", |m| m.as_str());
            let rest = caps.get(2).map_or("", |m| m.as_str());

            // Parse the bind arguments (comma-separated)
            let parts: Vec<&str> = rest.splitn(4, ',').map(|s| s.trim()).collect();

            if parts.len() >= 3 {
                let mut modifiers = parts[0].to_string();
                let key = parts[1].to_string();
                let action = parts[2].to_string();
                let argument = if parts.len() > 3 {
                    parts[3].to_string()
                } else {
                    String::new()
                };

                // Substitute variables in modifiers
                for (var, val) in &variables {
                    modifiers = modifiers.replace(var, val);
                }

                // Substitute variables in argument
                let mut arg_expanded = argument.clone();
                for (var, val) in &variables {
                    arg_expanded = arg_expanded.replace(var, val);
                }

                // Add bind type annotation for special binds
                let action_display = match bind_type {
                    "bindel" => format!("{} (repeat)", action),
                    "bindl" => format!("{} (locked)", action),
                    "bindm" => format!("{} (mouse)", action),
                    "bindd" => format!("{} (desc)", action),
                    _ => action,
                };

                let description = generate_description(&action_display, &arg_expanded);
                bindings.push(Keybinding {
                    modifiers,
                    key,
                    category: current_category.clone(),
                    description,
                });
            }
        }
    }

    // Ensure we have at least one category
    if categories.is_empty() {
        categories.push("General".to_string());
    }

    Ok((bindings, categories, shell))
}

/// Get bindings filtered by category
pub fn filter_by_category<'a>(bindings: &'a [Keybinding], category: &str) -> Vec<&'a Keybinding> {
    bindings.iter().filter(|b| b.category == category).collect()
}
