//! Forge - NixOS Configuration Tool
//! Copyright Cybex B.V.

mod app;
mod commands;
mod constants;
mod system;
mod templates;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    event::{
        read, DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use app::{App, AppMode, NixProgressEvent};
use commands::CommandMessage;

/// NixOS Configuration Tool
#[derive(Parser)]
#[command(name = "forge")]
#[command(author = "Cybex B.V.")]
#[command(version = "1.0.0")]
#[command(about = "NixOS Configuration Tool - TUI for install, update, and app profile management")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Fresh NixOS installation
    Install {
        /// Target hostname (for example: xps)
        hostname: Option<String>,
        /// Target disk device (e.g., /dev/nvme0n1)
        disk: Option<String>,
        /// Generate a hardware-detection layer from the live installer
        #[arg(long)]
        refresh_hardware: bool,
        /// Deprecated compatibility flag
        #[arg(long)]
        no_refresh_hardware: bool,
    },
    /// Create a new host configuration
    CreateHost {
        /// Hostname for the new configuration
        hostname: Option<String>,
    },
    /// Update flake inputs, rebuild system, and update CLI tools
    Update {
        #[command(subcommand)]
        action: Option<UpdateAction>,
        /// Only rebuild the system (skip flake update)
        #[arg(long, global = true)]
        rebuild_only: bool,
        /// Only update flake inputs (skip rebuild)
        #[arg(long, global = true)]
        flake_only: bool,
        /// Update specific flake inputs only
        #[arg(long = "input", global = true)]
        inputs: Vec<String>,
        /// Skip the NVIDIA driver compatibility check on kernel bumps
        #[arg(long, global = true)]
        skip_nvidia_check: bool,
        /// Show full command output, per-commit logs, and inline Nix warnings
        #[arg(long, global = true)]
        verbose: bool,
    },
    /// App profile management (browsers, Portal, etc.)
    #[command(alias = "browser")]
    Apps {
        #[command(subcommand)]
        action: Option<AppsAction>,
    },
    /// Key management (Age and SSH keys)
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
}

#[derive(Subcommand)]
enum UpdateAction {
    /// View update history
    History {
        /// Show details for specific update (by timestamp prefix)
        #[arg(long)]
        details: Option<String>,
    },
}

#[derive(Subcommand)]
enum AppsAction {
    /// Backup app profiles and push to GitHub
    Backup {
        /// Force backup even if apps are running
        #[arg(short, long)]
        force: bool,
    },
    /// Pull and restore app profiles from GitHub
    Restore {
        /// Force restore even if apps are running
        #[arg(short, long)]
        force: bool,
    },
    /// Check for app profile updates
    Status,
    /// Setup local SSH/Age keys from 1Password
    SetupKeys {
        /// Force overwrite of existing local keys
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum KeysAction {
    /// Setup keys from 1Password (one-time initial setup)
    Setup {
        /// Force overwrite of existing local keys
        #[arg(short, long)]
        force: bool,
    },
    /// Backup keys to passphrase-encrypted archive
    Backup,
    /// Restore keys from passphrase-encrypted archive
    Restore {
        /// Force overwrite of existing keys
        #[arg(short, long)]
        force: bool,
    },
    /// Show key status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging to file
    let log_dir = constants::forge_data_dir();
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, constants::FORGE_LOG_FILE);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    tracing::info!("Forge starting");

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Install {
            hostname,
            disk,
            refresh_hardware,
            no_refresh_hardware,
        }) => {
            run_tui(AppMode::Install(app::InstallState::new(
                hostname,
                disk,
                refresh_hardware && !no_refresh_hardware,
            )))
            .await
        }
        Some(Commands::CreateHost { hostname: _ }) => {
            // Hostname is now entered at the end of the wizard, so we always start with hardware detection
            run_tui(AppMode::CreateHost(app::CreateHostState::new())).await
        }
        Some(Commands::Update {
            action,
            rebuild_only,
            flake_only,
            inputs,
            skip_nvidia_check,
            verbose,
        }) => match action {
            Some(UpdateAction::History { details }) => {
                // Run history command directly (no TUI)
                commands::update::history::print_history(details.as_deref())?;
                Ok(())
            }
            None => {
                let options = app::UpdateOptions {
                    rebuild_only,
                    flake_only,
                    inputs,
                    presentation: app::UpdatePresentation::Modern,
                    skip_nvidia_check,
                };
                run_cli_update(options, verbose).await
            }
        },
        Some(Commands::Apps { action }) => match action {
            Some(AppsAction::Backup { force }) => {
                run_tui(AppMode::Apps(app::AppProfileState::new_backup(force))).await
            }
            Some(AppsAction::Restore { force }) => {
                run_tui(AppMode::Apps(app::AppProfileState::new_restore(force))).await
            }
            Some(AppsAction::Status) => {
                run_tui(AppMode::Apps(app::AppProfileState::new_status())).await
            }
            Some(AppsAction::SetupKeys { force }) => {
                run_tui(AppMode::Keys(app::KeysState::new_setup(force))).await
            }
            None => run_tui(AppMode::Apps(app::AppProfileState::new_menu())).await,
        },
        Some(Commands::Keys { action }) => match action {
            KeysAction::Setup { force } => {
                run_tui(AppMode::Keys(app::KeysState::new_setup(force))).await
            }
            KeysAction::Backup => run_tui(AppMode::Keys(app::KeysState::new_backup())).await,
            KeysAction::Restore { force } => {
                run_tui(AppMode::Keys(app::KeysState::new_restore(force))).await
            }
            KeysAction::Status => run_tui(AppMode::Keys(app::KeysState::new_status())).await,
        },
        None => run_tui(AppMode::MainMenu { selected: 0 }).await,
    }
}

async fn run_tui(initial_mode: AppMode) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(initial_mode);

    // Create command channel
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<CommandMessage>(constants::COMMAND_CHANNEL_SIZE);
    app.set_command_sender(cmd_tx);

    // Run the app
    let result = run_app(&mut terminal, &mut app, &mut cmd_rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Print log path
    println!("Screen log: {}", app.screen_log_path.display());

    if let Err(err) = result {
        eprintln!("Error: {err:?}");
        return Err(err);
    }

    Ok(())
}

async fn run_cli_update(options: app::UpdateOptions, verbose: bool) -> Result<()> {
    if let Some(error) = options.validate() {
        anyhow::bail!(error);
    }

    let started_at = Instant::now();
    print_cli_header();

    if !handle_cli_local_changes(&options)? {
        return Ok(());
    }

    let (tx, mut rx) = mpsc::channel::<CommandMessage>(constants::COMMAND_CHANNEL_SIZE);
    commands::update::start_update(tx, CancellationToken::new(), options).await?;

    let mut renderer = CliUpdateRenderer::new(verbose, started_at);
    let mut success = false;

    while let Some(msg) = rx.recv().await {
        match msg {
            CommandMessage::Stdout(line) => renderer.line(&line, false),
            CommandMessage::Stderr(line) => renderer.line(&line, true),
            CommandMessage::StepFailed { step, error } => {
                renderer.finish_progress_line();
                eprintln!("  {} {}: {}", failure("✗"), step, error.summary);
                if let Some(detail) = error.detail {
                    eprintln!("  {}", detail);
                }
                eprintln!("  {}", error.suggestion);
                renderer.dump_log();
            }
            CommandMessage::StepWarning { step, detail } => {
                renderer.finish_progress_line();
                if verbose {
                    println!("  {} {}: {}", warn("!"), step, detail);
                } else {
                    renderer.note(format!("{}: {}", step, detail));
                }
            }
            CommandMessage::StepSkipped { step, reason } => {
                renderer.step_skipped(&step, reason.as_deref());
            }
            CommandMessage::StepDetail { detail, .. } => {
                renderer.step_detail(&detail);
            }
            CommandMessage::StepComplete { .. } => {}
            CommandMessage::NixProgress(event) => renderer.nix_progress(event),
            CommandMessage::Done { success: done } => {
                renderer.finish_progress_line();
                success = done;
                break;
            }
            CommandMessage::Cancelled => {
                renderer.finish_progress_line();
                println!("  {}", warn("! update cancelled"));
                break;
            }
            CommandMessage::RollbackAvailable { generation } => {
                renderer.finish_progress_line();
                println!(
                    "{} rollback available to generation {}",
                    warn("!"),
                    generation
                );
            }
            CommandMessage::UpdateSummaryData { summary } => renderer.summary(summary),
            CommandMessage::UpdatePreflightReady { .. }
            | CommandMessage::UpdatePreflightFailed { .. }
            | CommandMessage::UpdatesAvailable { .. }
            | CommandMessage::CloneComplete { .. } => {}
        }
    }

    if success {
        renderer.done();
        Ok(())
    } else {
        anyhow::bail!("forge update failed")
    }
}

fn handle_cli_local_changes(options: &app::UpdateOptions) -> Result<bool> {
    let changes = commands::update::check_local_changes();
    if changes.is_empty() {
        render_section(
            "Pre-flight",
            &[format!("{} working tree clean", success("✓"))],
        );
        return Ok(true);
    }

    if commands::update::can_regenerate_dirty_flake_lock(options, &changes) {
        render_section(
            "Pre-flight",
            &[format!(
                "{} flake.lock has local changes; update will regenerate it",
                warn("!")
            )],
        );
        return Ok(true);
    }

    section("Pre-flight");
    let modified = changes.iter().filter(|change| change.tracked).count();
    let untracked = changes.len().saturating_sub(modified);
    let description = if untracked > 0 {
        format!(
            "Working tree dirty — {} modified, {} untracked",
            modified, untracked
        )
    } else {
        format!(
            "Working tree dirty — {} file{} modified",
            modified,
            if modified == 1 { "" } else { "s" }
        )
    };
    println!("  {} {}", warn("!"), description);
    for change in &changes {
        let marker = if change.tracked {
            "modified"
        } else {
            "untracked"
        };
        println!("  {:9} {}", marker, change.path);
    }
    match choose_preflight_action()? {
        CliPreflightAction::CommitAndPush => {
            println!(
                "  {} Codex: Commit and push all files to main/master",
                action("→")
            );
            let commit = commit_and_push_all_changes("Commit and push all files to main/master")?;
            println!("  {} Committed and pushed via Codex", success("✓"));
            println!("    {}", commit.subject);
            println!(
                "    {} · {} · {}",
                dim(&commit.hash),
                commit.shortstat,
                commit.pushed_ref
            );
            println!();
            Ok(true)
        }
        CliPreflightAction::Exit => {
            println!("  {} update cancelled", skipped("◦"));
            Ok(false)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliPreflightAction {
    Exit,
    CommitAndPush,
}

fn choose_preflight_action() -> Result<CliPreflightAction> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return choose_preflight_action_from_stdin();
    }

    let mut selected = CliPreflightAction::Exit;
    render_preflight_selector(selected)?;

    loop {
        if let Event::Key(key) = read_raw_event()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
                    selected = match selected {
                        CliPreflightAction::Exit => CliPreflightAction::CommitAndPush,
                        CliPreflightAction::CommitAndPush => CliPreflightAction::Exit,
                    };
                    redraw_preflight_selector(selected)?;
                }
                KeyCode::Enter => {
                    println!();
                    return Ok(selected);
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    println!();
                    return Ok(CliPreflightAction::Exit);
                }
                _ => {}
            }
        }
    }
}

fn choose_preflight_action_from_stdin() -> Result<CliPreflightAction> {
    println!();
    println!("  {} Choose how to continue", action("→"));
    println!("    {} Exit", dim("[1]"));
    println!("    {} Commit and push with Codex", dim("[2]"));
    println!();
    print!("  choice {} ", dim("›"));
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    println!();
    Ok(match choice.trim().to_ascii_lowercase().as_str() {
        "2" | "c" | "codex" | "commit" => CliPreflightAction::CommitAndPush,
        _ => CliPreflightAction::Exit,
    })
}

fn render_preflight_selector(selected: CliPreflightAction) -> Result<()> {
    println!();
    render_preflight_selector_body(selected)
}

fn render_preflight_selector_body(selected: CliPreflightAction) -> Result<()> {
    println!("  {} Choose how to continue", action("→"));
    print_preflight_option("Exit", selected == CliPreflightAction::Exit);
    print_preflight_option(
        "Commit and push with Codex",
        selected == CliPreflightAction::CommitAndPush,
    );
    println!("  {}", dim("↑/↓ select · Enter confirm · Esc exit"));
    io::stdout().flush()?;
    Ok(())
}

fn redraw_preflight_selector(selected: CliPreflightAction) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(
        stdout,
        MoveUp(4),
        MoveToColumn(0),
        Clear(ClearType::FromCursorDown)
    )?;
    render_preflight_selector_body(selected)
}

fn print_preflight_option(label: &str, selected: bool) {
    let marker = if selected { action("›") } else { dim(" ") };
    println!("    {} {}", marker, label);
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn read_raw_event() -> Result<Event> {
    let _raw_mode = RawModeGuard::new()?;
    Ok(read()?)
}

#[derive(Debug)]
struct CliCommitResult {
    hash: String,
    subject: String,
    shortstat: String,
    pushed_ref: String,
}

fn commit_and_push_all_changes(prompt: &str) -> Result<CliCommitResult> {
    let config_dir = constants::nixos_config_dir();
    let branch = git_output(&config_dir, &["branch", "--show-current"])?;
    if branch != "main" && branch != "master" {
        anyhow::bail!(
            "current branch is '{}'; switch to main or master before using this option",
            branch
        );
    }
    let before = git_output(&config_dir, &["rev-parse", "HEAD"]).ok();

    let mut codex = if constants::codex_cli_path().exists() {
        std::process::Command::new(constants::codex_cli_path())
    } else {
        std::process::Command::new("codex")
    };
    let output = codex
        .arg("exec")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("-C")
        .arg(&config_dir)
        .arg(prompt)
        .output()?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Codex failed to commit and push changes\n{}\n{}",
            stdout.trim(),
            stderr.trim()
        );
    }

    let branch = git_output(&config_dir, &["branch", "--show-current"])?;
    if branch != "main" && branch != "master" {
        anyhow::bail!("Codex left the repository on branch '{}'", branch);
    }

    let hash = git_output(&config_dir, &["rev-parse", "--short", "HEAD"])?;
    if before
        .as_deref()
        .map(|old| short_hash(old) == hash)
        .unwrap_or(false)
    {
        anyhow::bail!("Codex finished without creating a new commit");
    }

    let range = before
        .as_deref()
        .map(|old| format!("{}..HEAD", old))
        .unwrap_or_else(|| "HEAD~1..HEAD".to_string());
    let subject = git_output(&config_dir, &["log", "-1", "--format=%s"])?;
    let shortstat = git_output(&config_dir, &["diff", "--shortstat", &range])
        .unwrap_or_else(|_| "changes committed".to_string());
    run_git_quiet(&config_dir, &["push", "origin", &branch])?;
    let pushed_ref = format!("origin/{}", branch);
    let local_head = git_output(&config_dir, &["rev-parse", "HEAD"])?;
    let remote_head = git_output(&config_dir, &["rev-parse", &pushed_ref])?;
    if local_head != remote_head {
        anyhow::bail!(
            "push verification failed: HEAD does not match {}",
            pushed_ref
        );
    }
    let remaining = commands::update::check_local_changes();
    if !remaining.is_empty() {
        let paths = remaining
            .iter()
            .map(|change| change.path.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("Codex left uncommitted changes: {}", paths);
    }
    Ok(CliCommitResult {
        hash,
        subject,
        shortstat: normalize_shortstat(&shortstat),
        pushed_ref,
    })
}

fn run_git_quiet(config_dir: &std::path::Path, args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(config_dir)
        .args(args)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim())
    }
}

fn git_output(config_dir: &std::path::Path, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(config_dir)
        .args(args)
        .output()?;
    if !output.status.success() {
        anyhow::bail!("git {} failed", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn print_cli_header() {
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let left = "forge update";
    let right = format!("{} · {}", host, nixos_version_label());
    let width = term_width();
    let used = left.chars().count() + right.chars().count();
    let pad = width.saturating_sub(used).max(1);
    println!("{}{}{}", emphasize(left), " ".repeat(pad), dim(&right));
    println!("{}", rule());
    println!();
}

struct CliUpdateRenderer {
    verbose: bool,
    started_at: Instant,
    shown_sections: HashSet<&'static str>,
    notes: Vec<String>,
    log_buffer: Vec<String>,
    build_seen: HashSet<String>,
    build_done: HashSet<String>,
    progress_active: bool,
    spinner_frame: usize,
    live_activity: Option<String>,
    flake_note: Option<String>,
}

impl CliUpdateRenderer {
    fn new(verbose: bool, started_at: Instant) -> Self {
        Self {
            verbose,
            started_at,
            shown_sections: HashSet::new(),
            notes: Vec::new(),
            log_buffer: Vec::new(),
            build_seen: HashSet::new(),
            build_done: HashSet::new(),
            progress_active: false,
            spinner_frame: 0,
            live_activity: None,
            flake_note: None,
        }
    }

    fn line(&mut self, line: &str, stderr: bool) {
        if !line.trim().is_empty() {
            self.log_buffer.push(line.to_string());
        }

        if line.trim().is_empty() || should_suppress_default_line(line) {
            return;
        }

        if !self.verbose {
            if is_nix_warning(line) {
                self.note(strip_ansi(line).trim().to_string());
            }
            return;
        }

        self.finish_progress_line();
        if stderr {
            eprintln!("{}", line);
        } else {
            println!("{}", line);
        }
    }

    fn step_skipped(&mut self, step: &str, reason: Option<&str>) {
        match step {
            "update.pull" => self.inline_skipped(
                "Pulling configuration",
                reason.unwrap_or("already up to date"),
            ),
            "update.rebuild" => {
                self.inline_skipped("Rebuilding system", reason.unwrap_or("no changes"))
            }
            "update.packages" => {
                self.inline_skipped("Comparing packages", reason.unwrap_or("no changes"))
            }
            _ if self.verbose => {
                self.finish_progress_line();
                println!(
                    "  {} {} {}",
                    skipped("◦"),
                    step,
                    dim(reason.unwrap_or("skipped"))
                );
            }
            _ => {}
        }
    }

    fn step_detail(&mut self, detail: &str) {
        if self.verbose {
            self.finish_progress_line();
            println!("  {} {}", action("→"), detail);
        }
    }

    fn nix_progress(&mut self, event: NixProgressEvent) {
        match event {
            NixProgressEvent::Section { .. } => {
                self.activity("rebuilding system");
            }
            NixProgressEvent::Building { name } => {
                self.build_seen.insert(name);
                if self.live_activity.as_deref() == Some("rebuilding system") {
                    self.draw_live();
                } else {
                    self.activity("rebuilding system");
                }
            }
            NixProgressEvent::Complete { name } => {
                if name.ends_with(".drv") {
                    self.build_done.insert(name);
                    self.draw_live();
                }
            }
            NixProgressEvent::Failed { name } => {
                self.finish_progress_line();
                println!("  {} {}", failure("✗"), name);
            }
            NixProgressEvent::Activating => {
                self.activity("activating new system generation");
            }
            NixProgressEvent::Download { .. } => {}
        }
    }

    fn summary(&mut self, summary: app::UpdateSummary) {
        self.finish_progress_line();

        for warning in &summary.follow_up_warnings {
            self.note(warning.clone());
        }
        self.render_flake_summary(&summary);
        self.render_rebuild_summary(&summary);
        self.render_post_update(&summary);
        self.render_notes();
        self.render_final_summary(&summary);
    }

    fn done(&mut self) {
        println!();
        println!("{}", success("✓ done."));
    }

    fn render_flake_summary(&mut self, summary: &app::UpdateSummary) {
        let name_w = summary
            .flake_changes
            .iter()
            .map(|change| change.name.chars().count())
            .max()
            .unwrap_or(0)
            .max(8);
        let mut items = Vec::new();
        for change in &summary.flake_changes {
            let old = short_hash(&change.old_rev);
            let new = short_hash(&change.new_rev);
            let commits = if change.total_commits == 0 {
                "unknown commits".to_string()
            } else {
                format!(
                    "{} commit{}",
                    format_count(change.total_commits),
                    if change.total_commits == 1 { "" } else { "s" }
                )
            };
            let age = change
                .new_last_modified
                .map(relative_time)
                .unwrap_or_else(|| "unknown".to_string());
            items.push(format!(
                "{} {:<width$}  {} {} {}   {:<13} {}",
                upgrade("↑"),
                change.name,
                dim(&old),
                dim("→"),
                new,
                commits,
                dim(&age),
                width = name_w,
            ));
            if self.verbose {
                for commit in &change.commits {
                    items.push(format!("{} {}", dim(&commit.hash), commit.message));
                }
                if let Some(url) = &change.compare_url {
                    items.push(dim(url));
                }
            }
        }
        if let Some(note) = self.flake_note.take() {
            items.push(note);
        }
        self.section_block("Flake inputs", items);
    }

    fn render_rebuild_summary(&mut self, summary: &app::UpdateSummary) {
        if matches!(
            summary.core_status,
            app::UpdateCoreStatus::Success | app::UpdateCoreStatus::UpToDate
        ) {
            let generation = summary
                .system_after
                .as_deref()
                .map(short_store_hash)
                .unwrap_or_else(|| "current".to_string());
            self.section_block(
                "Rebuilding system",
                vec![format!("{} activated generation {}", success("✓"), generation)],
            );
        }
    }

    fn render_post_update(&mut self, summary: &app::UpdateSummary) {
        let mut items = Vec::new();
        items.extend(tool_version_item(
            "claude code",
            &summary.claude_old,
            &summary.claude_new,
        ));
        items.extend(tool_version_item(
            "codex cli",
            &summary.codex_old,
            &summary.codex_new,
        ));
        items.extend(status_line_item("browser profiles", &summary.browser_status));
        items.extend(status_line_item("firmware", &summary.firmware_status));
        self.section_block("Post-update", items);
    }

    fn render_notes(&mut self) {
        let items: Vec<String> = self
            .notes
            .iter()
            .map(|note| format!("{} {}", warn("!"), note))
            .collect();
        self.section_block("Notes", items);
    }

    fn render_final_summary(&self, summary: &app::UpdateSummary) {
        println!("{}", rule());
        if let Some(closure) = &summary.closure_summary {
            summary_row("closure", closure);
        }
        summary_row(
            "duration",
            &format_duration(self.started_at.elapsed().as_secs()),
        );
        let snapshot = match (&summary.system_before, &summary.system_after) {
            (Some(before), Some(after)) if before != after => "before/after captured",
            (Some(_), Some(_)) => "before/after unchanged",
            _ => "unavailable",
        };
        summary_row("snapshot", snapshot);
    }

    /// Render a complete section (header + tree-rail items) once. Re-rendering the
    /// same title is a no-op so a phase shown live (e.g. a skip) isn't duplicated by
    /// the summary pass.
    fn section_block(&mut self, title: &'static str, items: Vec<String>) {
        if items.is_empty() || !self.shown_sections.insert(title) {
            return;
        }
        self.finish_progress_line();
        render_section(title, &items);
    }

    fn inline_skipped(&mut self, title: &'static str, reason: &str) {
        let item = format!(
            "{} {}",
            skipped("◦"),
            dim(&format!("skipped — {}", reason.trim()))
        );
        self.section_block(title, vec![item]);
    }

    fn note(&mut self, note: String) {
        if !note.is_empty() && !self.notes.iter().any(|existing| existing == &note) {
            self.notes.push(note);
        }
    }

    /// Set the current live activity and redraw the transient status line. On a TTY
    /// this animates a spinner (and a build bar when derivations are in flight); when
    /// piped it falls back to one plain line per distinct activity.
    fn activity(&mut self, text: &str) {
        if !progress_enabled() {
            if self.live_activity.as_deref() != Some(text) {
                self.live_activity = Some(text.to_string());
                println!("  {} {}", action("→"), text);
            }
            return;
        }
        self.live_activity = Some(text.to_string());
        self.draw_live();
    }

    fn draw_live(&mut self) {
        if !progress_enabled() {
            return;
        }
        let frame = SPINNER_CHARS[self.spinner_frame % SPINNER_CHARS.len()];
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        let activity = self.live_activity.clone().unwrap_or_default();
        let mut line = format!("  {} {}", accent(&frame.to_string()), activity);
        if !self.build_seen.is_empty() {
            let total = self.build_seen.len().max(1);
            let done = self.build_done.len().min(total);
            let (filled, empty, _) = progress_parts(done as u64, total as u64, 20);
            line.push_str(&format!(
                "  {}{}  {}/{}",
                "█".repeat(filled),
                dim(&"░".repeat(empty)),
                done,
                total
            ));
        }
        print!("\r\x1b[K{}", line);
        let _ = io::stdout().flush();
        self.progress_active = true;
    }

    fn dump_log(&mut self) {
        if self.log_buffer.is_empty() {
            return;
        }
        eprintln!();
        eprintln!("{}", header_text("Verbose log"));
        for line in &self.log_buffer {
            eprintln!("{}", line);
        }
    }

    fn finish_progress_line(&mut self) {
        if self.progress_active {
            print!("\r\x1b[K");
            let _ = io::stdout().flush();
            self.progress_active = false;
        }
    }
}

fn tool_version_item(label: &str, old: &Option<String>, new: &Option<String>) -> Option<String> {
    match (old.as_deref(), new.as_deref()) {
        (Some(old), Some(new)) if old != new => Some(format!(
            "{} {:18} {} {} {}",
            success("✓"),
            label,
            dim(old),
            dim("→"),
            new
        )),
        (Some(_), Some(new)) => Some(format!("{} {:32} {}", skipped("◦"), label, new)),
        _ => None,
    }
}

fn status_line_item(label: &str, status: &str) -> Option<String> {
    if status.is_empty() {
        return None;
    }
    if status.contains("failed") || status.contains("available") {
        Some(format!("{} {:32} {}", warn("!"), label, status))
    } else {
        Some(format!("{} {:32} {}", skipped("◦"), label, status))
    }
}

fn summary_row(label: &str, value: &str) {
    let label = format!("{:<10}", label);
    println!(" {} {}", dim(&label), value);
}

/// Print a section header followed by its items hung under a dim `├`/`└` tree rail.
fn render_section(title: &str, items: &[String]) {
    section(title);
    let last = items.len().saturating_sub(1);
    for (idx, item) in items.iter().enumerate() {
        let connector = if idx == last { "└" } else { "├" };
        println!("  {} {}", dim(connector), item);
    }
    println!();
}

fn section(title: &str) {
    println!("{}", header_text(title));
}

fn header_text(title: &str) -> String {
    format!("{} {}", accent("❯"), emphasize(title))
}

fn should_suppress_default_line(line: &str) -> bool {
    let trimmed = strip_ansi(line);
    let trimmed = trimmed.trim();
    trimmed.is_empty()
        || trimmed
            .chars()
            .all(|c| matches!(c, '=' | '═' | '╔' | '╗' | '╚' | '╝' | '─' | ' '))
        || trimmed.contains("NixOS System Update")
        || trimmed.contains("Updating Flake Inputs")
        || trimmed.contains("Rebuilding System")
        || trimmed.contains("Update Summary")
        || trimmed.starts_with("Closure:")
        || trimmed.starts_with("System:")
        || trimmed.starts_with("Snapshot:")
}

fn is_nix_warning(line: &str) -> bool {
    strip_ansi(line).to_ascii_lowercase().contains("warning:")
}

fn strip_ansi(value: &str) -> String {
    static ANSI: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());
    ANSI.replace_all(value, "").to_string()
}

fn normalize_shortstat(value: &str) -> String {
    let mut parts = Vec::new();
    for segment in value.trim().split(',').map(str::trim) {
        if segment.contains("insertion") {
            if let Some(count) = segment.split_whitespace().next() {
                parts.push(format!("+{}", count));
            }
        } else if segment.contains("deletion") {
            if let Some(count) = segment.split_whitespace().next() {
                parts.push(format!("−{}", count));
            }
        }
    }

    if parts.is_empty() {
        value.trim().to_string()
    } else {
        parts.join(" ")
    }
}

fn nixos_version_label() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                let value = line.strip_prefix("VERSION_ID=")?;
                Some(format!("nixos {}", value.trim_matches('"')))
            })
        })
        .unwrap_or_else(|| "nixos".to_string())
}

fn short_hash(value: &str) -> String {
    value.chars().take(7).collect()
}

fn short_store_hash(value: &str) -> String {
    value
        .rsplit('/')
        .next()
        .and_then(|name| name.split('-').next())
        .map(short_hash)
        .unwrap_or_else(|| short_hash(value))
}

fn format_count(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn format_duration(seconds: u64) -> String {
    if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

fn relative_time(unix_seconds: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let elapsed = now.saturating_sub(unix_seconds).max(0);
    let days = elapsed / 86_400;
    if days > 0 {
        format!("{}d ago", days)
    } else {
        let hours = elapsed / 3_600;
        if hours > 0 {
            format!("{}h ago", hours)
        } else {
            "today".to_string()
        }
    }
}

/// Braille spinner frames for the transient live status line (matches the TUI spinner).
const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn color_enabled() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn progress_enabled() -> bool {
    color_enabled()
}

/// Usable terminal width for rules and right-aligned labels. Falls back to a fixed
/// width when output isn't a TTY so piped logs stay stable.
fn term_width() -> usize {
    if !io::stdout().is_terminal() {
        return 58;
    }
    crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(58)
        .clamp(40, 100)
}

fn rule() -> String {
    dim(&"─".repeat(term_width()))
}

fn style(code: &str, text: &str) -> String {
    if color_enabled() {
        format!("\x1b[{}m{}\x1b[0m", code, text)
    } else {
        text.to_string()
    }
}

fn accent(text: &str) -> String {
    style("36;1", text)
}

fn emphasize(text: &str) -> String {
    style("1", text)
}

fn success(text: &str) -> String {
    style("32;1", text)
}

fn upgrade(text: &str) -> String {
    style("34;1", text)
}

fn action(text: &str) -> String {
    style("34;1", text)
}

fn skipped(text: &str) -> String {
    dim(text)
}

fn warn(text: &str) -> String {
    style("33;1", text)
}

fn failure(text: &str) -> String {
    style("31;1", text)
}

fn dim(text: &str) -> String {
    style("2", text)
}

fn progress_parts(transferred: u64, total: u64, width: usize) -> (usize, usize, u64) {
    if total == 0 || width == 0 {
        return (0, width, 0);
    }
    let clamped = transferred.min(total);
    let filled = ((clamped as f64 / total as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    let percent = ((clamped as f64 / total as f64) * 100.0).round() as u64;
    (filled, width.saturating_sub(filled), percent.min(100))
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    cmd_rx: &mut mpsc::Receiver<CommandMessage>,
) -> Result<()> {
    // Start any initial commands based on mode
    app.start_initial_command().await?;

    // Create async event stream for responsive input
    let mut event_stream = EventStream::new();

    loop {
        // Draw UI
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Handle all events with proper async - no blocking delays
        let timeout = Duration::from_millis(constants::EVENT_POLL_TIMEOUT_MS);

        tokio::select! {
            biased;  // Prioritize in order: keys, commands, timeout

            // Terminal key events (instant response)
            Some(Ok(event)) = event_stream.next() => {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        app.handle_key(key).await?;
                    }
                }
            }
            // Command output from async tasks
            Some(msg) = cmd_rx.recv() => {
                app.handle_command_message(msg).await?;
            }
            // Timeout for spinner animation and redraw
            _ = tokio::time::sleep(timeout) => {}
        }

        // Update spinner animation
        app.tick();

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
