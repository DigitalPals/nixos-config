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
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::time::Duration;
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
}

#[derive(Subcommand)]
enum KeysAction {
    /// Setup keys from 1Password (one-time initial setup)
    Setup,
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
                };
                run_cli_update(options).await
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
            None => run_tui(AppMode::Apps(app::AppProfileState::new_menu())).await,
        },
        Some(Commands::Keys { action }) => match action {
            KeysAction::Setup => run_tui(AppMode::Keys(app::KeysState::new_setup())).await,
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

async fn run_cli_update(options: app::UpdateOptions) -> Result<()> {
    if let Some(error) = options.validate() {
        anyhow::bail!(error);
    }

    if !handle_cli_local_changes()? {
        return Ok(());
    }

    let (tx, mut rx) = mpsc::channel::<CommandMessage>(constants::COMMAND_CHANNEL_SIZE);
    commands::update::start_update(tx, CancellationToken::new(), options).await?;

    let mut renderer = CliUpdateRenderer::default();
    let mut success = false;

    while let Some(msg) = rx.recv().await {
        match msg {
            CommandMessage::Stdout(line) => renderer.line(&line, false),
            CommandMessage::Stderr(line) => renderer.line(&line, true),
            CommandMessage::StepFailed { step, error } => {
                renderer.finish_progress_line();
                eprintln!("{} {}: {}", ansi_red("error"), step, error.summary);
                if let Some(detail) = error.detail {
                    eprintln!("  {}", detail);
                }
                eprintln!("  {}", error.suggestion);
            }
            CommandMessage::StepWarning { step, detail } => {
                renderer.finish_progress_line();
                println!("{} {}: {}", ansi_yellow("warning"), step, detail);
            }
            CommandMessage::StepSkipped { step, reason } => {
                renderer.finish_progress_line();
                if let Some(reason) = reason {
                    println!("{} {} ({})", ansi_dim("skipped"), step, reason);
                }
            }
            CommandMessage::StepDetail { detail, .. } => {
                renderer.finish_progress_line();
                println!("{} {}", ansi_cyan("::"), detail);
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
                println!("{}", ansi_yellow("Update cancelled."));
                break;
            }
            CommandMessage::RollbackAvailable { generation } => {
                renderer.finish_progress_line();
                println!(
                    "{} rollback available to generation {}",
                    ansi_yellow("warning"),
                    generation
                );
            }
            CommandMessage::UpdateSummaryData { .. }
            | CommandMessage::UpdatePreflightReady { .. }
            | CommandMessage::UpdatePreflightFailed { .. }
            | CommandMessage::UpdatesAvailable { .. }
            | CommandMessage::CloneComplete { .. } => {}
        }
    }

    if success {
        Ok(())
    } else {
        anyhow::bail!("forge update failed")
    }
}

fn handle_cli_local_changes() -> Result<bool> {
    let changes = commands::update::check_local_changes();
    if changes.is_empty() {
        return Ok(true);
    }

    println!("{}", ansi_yellow("Uncommitted files detected:"));
    for change in &changes {
        let marker = if change.tracked {
            "modified"
        } else {
            "untracked"
        };
        println!("  {:9} {}", marker, change.path);
    }
    println!();
    println!("1) Exit");
    println!("2) Use Codex to commit and push");
    print!("Select an option [1/2]: ");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    match choice.trim() {
        "2" => {
            println!("Codex prompt: Commit and push all files to main/master");
            commit_and_push_all_changes("Commit and push all files to main/master")?;
            Ok(true)
        }
        _ => {
            println!("Exiting without updating.");
            Ok(false)
        }
    }
}

fn commit_and_push_all_changes(description: &str) -> Result<()> {
    let config_dir = constants::nixos_config_dir();
    let branch = git_output(&config_dir, &["branch", "--show-current"])?;
    if branch != "main" && branch != "master" {
        anyhow::bail!(
            "current branch is '{}'; switch to main or master before using this option",
            branch
        );
    }

    run_git(&config_dir, &["add", "-A"])?;
    run_git(&config_dir, &["commit", "-m", description])?;
    run_git(&config_dir, &["push", "origin", &branch])?;
    println!(
        "{} committed and pushed to origin/{}",
        ansi_green("✓"),
        branch
    );
    Ok(())
}

fn run_git(config_dir: &std::path::Path, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(config_dir)
        .args(args)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("git {} failed", args.join(" "))
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

#[derive(Default)]
struct CliUpdateRenderer {
    progress_active: bool,
}

impl CliUpdateRenderer {
    fn line(&mut self, line: &str, stderr: bool) {
        if line.trim().is_empty() {
            return;
        }
        self.finish_progress_line();
        if stderr {
            eprintln!("{}", line);
        } else {
            println!("{}", line);
        }
    }

    fn nix_progress(&mut self, event: NixProgressEvent) {
        match event {
            NixProgressEvent::Section { title } => {
                self.finish_progress_line();
                println!("{} {}", ansi_cyan("::"), ansi_cyan(&title));
            }
            NixProgressEvent::Download {
                name,
                transferred,
                total,
                speed_bps,
                eta_secs,
            } => self.render_download(&name, transferred, total, speed_bps, eta_secs),
            NixProgressEvent::Building { name } => {
                self.finish_progress_line();
                println!(
                    "{:<34} {}",
                    truncate_name(&name, 34),
                    ansi_yellow("building")
                );
            }
            NixProgressEvent::Activating => {
                self.finish_progress_line();
                println!("{}", ansi_cyan(":: Activating new system generation..."));
            }
            NixProgressEvent::Complete { name } => {
                self.finish_progress_line();
                println!(
                    "{:<34} {}",
                    truncate_name(&name, 34),
                    ansi_green("complete")
                );
            }
            NixProgressEvent::Failed { name } => {
                self.finish_progress_line();
                println!("{:<34} {}", truncate_name(&name, 34), ansi_red("failed"));
            }
        }
    }

    fn render_download(
        &mut self,
        name: &str,
        transferred: u64,
        total: Option<u64>,
        speed_bps: Option<f64>,
        eta_secs: Option<u64>,
    ) {
        let row = if let Some(total) = total {
            let (filled, empty, percent) = progress_parts(transferred, total, 28);
            format!(
                "\r{:<34} {:>9} {:>11} {:>6} [{}{}] {:>3}%",
                truncate_name(name, 34),
                format_bytes(total),
                speed_bps
                    .map(format_speed)
                    .unwrap_or_else(|| "--".to_string()),
                eta_secs
                    .map(format_eta)
                    .unwrap_or_else(|| "--:--".to_string()),
                ansi_green(&"█".repeat(filled)),
                ansi_dim(&"░".repeat(empty)),
                percent
            )
        } else {
            format!(
                "\r{:<34} {:>11} {:>9} {:>11}",
                truncate_name(name, 34),
                ansi_cyan("downloading"),
                format_bytes(transferred),
                speed_bps
                    .map(format_speed)
                    .unwrap_or_else(|| "--".to_string())
            )
        };
        print!("{}", row);
        let _ = io::stdout().flush();
        self.progress_active = true;
    }

    fn finish_progress_line(&mut self) {
        if self.progress_active {
            println!();
            self.progress_active = false;
        }
    }
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

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{} B", bytes)
    }
}

fn format_speed(bytes_per_second: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_second.max(0.0) as u64))
}

fn format_eta(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn truncate_name(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let head_len = max_len - 3;
    let head: String = value.chars().take(head_len).collect();
    format!("{}...", head)
}

fn ansi_cyan(text: &str) -> String {
    format!("\x1b[36;1m{}\x1b[0m", text)
}

fn ansi_green(text: &str) -> String {
    format!("\x1b[32;1m{}\x1b[0m", text)
}

fn ansi_yellow(text: &str) -> String {
    format!("\x1b[33;1m{}\x1b[0m", text)
}

fn ansi_red(text: &str) -> String {
    format!("\x1b[31;1m{}\x1b[0m", text)
}

fn ansi_dim(text: &str) -> String {
    format!("\x1b[2m{}\x1b[0m", text)
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
