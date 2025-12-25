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
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use app::{App, AppMode};
use commands::CommandMessage;

/// NixOS Configuration Tool
#[derive(Parser)]
#[command(name = "forge")]
#[command(author = "Cybex B.V.")]
#[command(version = "1.0.0")]
#[command(about = "NixOS Configuration Tool - TUI for install, update, and browser profile management")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Fresh NixOS installation
    Install {
        /// Target hostname (kraken or G1a)
        hostname: Option<String>,
        /// Target disk device (e.g., /dev/nvme0n1)
        disk: Option<String>,
    },
    /// Create a new host configuration
    CreateHost {
        /// Hostname for the new configuration
        hostname: Option<String>,
    },
    /// Update flake inputs, rebuild system, and update CLI tools
    Update,
    /// Browser profile management
    Browser {
        #[command(subcommand)]
        action: Option<BrowserAction>,
    },
}

#[derive(Subcommand)]
enum BrowserAction {
    /// Backup browser profiles and push to GitHub
    Backup {
        /// Force backup even if browsers are running
        #[arg(short, long)]
        force: bool,
    },
    /// Pull and restore browser profiles from GitHub
    Restore {
        /// Force restore even if browsers are running
        #[arg(short, long)]
        force: bool,
    },
    /// Check for browser profile updates
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging to file
    let log_dir = dirs::home_dir()
        .map(|h| h.join(".local/share/forge"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/forge"));
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "forge.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    tracing::info!("Forge starting");

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Install { hostname, disk }) => {
            run_tui(AppMode::Install(app::InstallState::new(hostname, disk))).await
        }
        Some(Commands::CreateHost { hostname: _ }) => {
            // Hostname is now entered at the end of the wizard, so we always start with hardware detection
            run_tui(AppMode::CreateHost(app::CreateHostState::new())).await
        }
        Some(Commands::Update) => run_tui(AppMode::Update(app::UpdateState::new())).await,
        Some(Commands::Browser { action }) => match action {
            Some(BrowserAction::Backup { force }) => {
                run_tui(AppMode::Browser(app::BrowserState::new_backup(force))).await
            }
            Some(BrowserAction::Restore { force }) => {
                run_tui(AppMode::Browser(app::BrowserState::new_restore(force))).await
            }
            Some(BrowserAction::Status) => {
                run_tui(AppMode::Browser(app::BrowserState::new_status())).await
            }
            None => run_tui(AppMode::Browser(app::BrowserState::new_menu())).await,
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
                        app.handle_key(key.code).await?;
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
