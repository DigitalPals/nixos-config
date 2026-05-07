//! Command message handling for the application

use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

use std::time::Instant;

use super::state::{
    AppMode, AppProfileState, CommitInfo, CreateHostState, InstallState, KeysState,
    NixProgressEvent, NixProgressRow, NixProgressStatus, StepState, StepStatus, UpdateState,
};
use super::App;
use crate::commands::errors::ParsedError;
use crate::commands::executor::run_capture;
use crate::commands::CommandMessage;
use crate::constants::{nixos_config_dir, OUTPUT_BUFFER_SIZE};

/// Regex to match ANSI escape codes.
static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    ANSI_RE.replace_all(s, "").to_string()
}

fn upsert_progress_row(rows: &mut Vec<NixProgressRow>, row: NixProgressRow) {
    if let Some(existing) = rows.iter_mut().find(|existing| existing.name == row.name) {
        *existing = row;
    } else {
        rows.push(row);
    }
}

impl App {
    /// Handle messages from running commands
    pub async fn handle_command_message(&mut self, msg: CommandMessage) -> Result<()> {
        match msg {
            CommandMessage::Stdout(line) | CommandMessage::Stderr(line) => {
                self.append_output(&line);
            }
            CommandMessage::StepComplete { step } => {
                self.mark_step_complete(&step);
            }
            CommandMessage::StepFailed { step, error } => {
                self.mark_step_failed(&step, error);
            }
            CommandMessage::StepWarning { step, detail } => {
                self.mark_step_warning(&step, &detail);
            }
            CommandMessage::StepSkipped { step, reason } => {
                self.mark_step_skipped(&step, reason.as_deref());
            }
            CommandMessage::StepDetail { step, detail } => {
                self.set_step_detail(&step, &detail);
            }
            CommandMessage::NixProgress(event) => {
                self.apply_nix_progress(event);
            }
            CommandMessage::Done { success } => {
                self.handle_command_done(success).await;
            }
            CommandMessage::Cancelled => {
                self.handle_command_cancelled();
            }
            CommandMessage::UpdatesAvailable {
                nixos_config,
                app_profiles,
                commits,
            } => {
                self.startup_check_running = false;
                if (nixos_config || app_profiles) && matches!(self.mode, AppMode::MainMenu { .. }) {
                    self.pending_updates.nixos_config = nixos_config;
                    self.pending_updates.app_profiles = app_profiles;
                    self.pending_updates.commits = commits
                        .into_iter()
                        .map(|(hash, message)| CommitInfo { hash, message })
                        .collect();
                    self.pending_updates.selected = 0;
                    self.pending_updates.viewing_commits = false;
                    self.pending_updates.commit_scroll = 0;
                }
            }
            CommandMessage::RollbackAvailable { generation } => {
                self.push_modal(super::ModalDialog::RollbackPrompt {
                    generation,
                    selected: 0,
                });
            }
            CommandMessage::CloneComplete { success } => {
                self.handle_clone_complete(success);
            }
            CommandMessage::UpdateSummaryData { summary } => {
                // Store the summary for use when Done arrives
                self.update_summary = Some(summary);
            }
            CommandMessage::UpdatePreflightReady { report } => {
                self.handle_update_preflight_ready(report).await?;
            }
            CommandMessage::UpdatePreflightFailed { error } => {
                if let AppMode::Update(UpdateState::Preparing { options, .. }) = &self.mode {
                    self.mode = AppMode::Update(UpdateState::preflight(options.clone(), false));
                }
                self.error = Some(error);
            }
        }
        Ok(())
    }

    fn handle_clone_complete(&mut self, success: bool) {
        if success {
            // Re-discover hosts from the newly cloned repository
            self.hosts = crate::system::config::discover_hosts();
            // Transition to host selection
            self.mode = AppMode::Install(InstallState::SelectHost { selected: 0 });
        } else {
            // Clone failed - show error screen
            use std::collections::VecDeque;
            let mut output = VecDeque::new();
            output.push_back("Failed to clone configuration repository.".to_string());
            output.push_back("".to_string());
            output.push_back("Please check:".to_string());
            output
                .push_back("  1. Internet connection (run 'nmtui' to configure WiFi)".to_string());
            output.push_back("  2. GitHub is accessible".to_string());
            output.push_back("".to_string());
            output.push_back("Press Enter to return to main menu.".to_string());
            self.mode = AppMode::Install(InstallState::Complete {
                success: false,
                output,
                scroll_offset: None,
            });
        }
    }

    fn append_output(&mut self, line: &str) {
        let clean_line = strip_ansi_codes(line);
        self.log_to_screen(&clean_line);

        match &mut self.mode {
            AppMode::Update(UpdateState::Running { output, .. })
            | AppMode::Update(UpdateState::Preparing { output, .. })
            | AppMode::Update(UpdateState::Complete { output, .. }) => {
                output.push_back(clean_line);
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Install(InstallState::CloneRepository { output }) => {
                output.push_back(clean_line);
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Install(InstallState::Running { output, .. }) => {
                output.push_back(clean_line);
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Apps(AppProfileState::Running { output, .. }) => {
                output.push_back(clean_line);
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Apps(AppProfileState::Status { output }) => {
                output.push_back(clean_line);
            }
            AppMode::Keys(KeysState::Running { output, .. }) => {
                output.push_back(clean_line);
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::CreateHost(CreateHostState::Generating { output, .. }) => {
                output.push_back(clean_line);
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            _ => {}
        }
    }

    fn find_step_mut<'a>(steps: &'a mut [StepStatus], step_id: &str) -> Option<&'a mut StepStatus> {
        steps.iter_mut().find(|step| step.id == step_id)
    }

    fn mark_step_complete(&mut self, step_id: &str) {
        self.log_to_screen(&format!("[✓] Step complete: {}", step_id));

        match &mut self.mode {
            AppMode::Update(UpdateState::Preparing { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Complete;
                    s.detail = None;
                }
                if let Some(next_index) = steps
                    .iter()
                    .position(|step| step.id == step_id)
                    .and_then(|index| (index + 1 < steps.len()).then_some(index + 1))
                {
                    steps[next_index].status = StepState::Running;
                    steps[next_index].started_at = Some(Instant::now());
                }
            }
            AppMode::Update(UpdateState::Running { steps, step, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Complete;
                    s.detail = None;
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                    steps[*step].started_at = Some(Instant::now());
                }
                // Save progress for resume
                super::resume::save_pending("update", *step, steps.len());
            }
            AppMode::Install(InstallState::Running { steps, step, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Complete;
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                    steps[*step].started_at = Some(Instant::now());
                }
            }
            AppMode::CreateHost(CreateHostState::Generating { steps, step, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Complete;
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                    steps[*step].started_at = Some(Instant::now());
                }
            }
            _ => {}
        }
    }

    fn mark_step_failed(&mut self, step_id: &str, error: ParsedError) {
        // Log formatted error to screen
        self.log_to_screen(&format!("[✗] Step failed: {}", step_id));
        self.log_to_screen("");
        self.log_to_screen(&format!("  Error: {}", error.summary));
        if let Some(ref detail) = error.detail {
            for line in detail.lines() {
                self.log_to_screen(&format!("  {}", line));
            }
        }
        self.log_to_screen("");
        self.log_to_screen(&format!("  Suggestion: {}", error.suggestion));

        match &mut self.mode {
            AppMode::Update(UpdateState::Preparing { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Failed;
                    s.detail = error.detail.clone();
                }
                self.error = Some(error.summary);
            }
            AppMode::Update(UpdateState::Running { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Failed;
                }
                self.error = Some(error.summary);
            }
            AppMode::Install(InstallState::Running { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Failed;
                }
                self.error = Some(error.summary);
            }
            AppMode::CreateHost(CreateHostState::Generating { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Failed;
                }
                self.error = Some(error.summary);
            }
            _ => {}
        }
    }

    fn mark_step_warning(&mut self, step_id: &str, detail: &str) {
        self.log_to_screen(&format!("[!] Step warning: {}", step_id));
        self.log_to_screen(&format!("  {}", detail));

        if let AppMode::Update(UpdateState::Preparing { steps, .. }) = &mut self.mode {
            if let Some(s) = Self::find_step_mut(steps, step_id) {
                s.status = StepState::Warning;
                s.detail = Some(detail.to_string());
            }
            if let Some(next_index) = steps
                .iter()
                .position(|step| step.id == step_id)
                .and_then(|index| (index + 1 < steps.len()).then_some(index + 1))
            {
                steps[next_index].status = StepState::Running;
                steps[next_index].started_at = Some(Instant::now());
            }
        } else if let AppMode::Update(UpdateState::Running { steps, step, .. }) = &mut self.mode {
            if let Some(s) = Self::find_step_mut(steps, step_id) {
                s.status = StepState::Warning;
                s.detail = Some(detail.to_string());
            }
            *step = (*step + 1).min(steps.len());
            if *step < steps.len() {
                steps[*step].status = StepState::Running;
                steps[*step].started_at = Some(Instant::now());
            }
        }
    }

    fn set_step_detail(&mut self, step_id: &str, detail: &str) {
        match &mut self.mode {
            AppMode::Update(UpdateState::Preparing { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.detail = Some(detail.to_string());
                }
            }
            AppMode::Update(UpdateState::Running { steps, .. })
            | AppMode::Install(InstallState::Running { steps, .. })
            | AppMode::CreateHost(CreateHostState::Generating { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.detail = Some(detail.to_string());
                }
            }
            _ => {}
        }
    }

    fn apply_nix_progress(&mut self, event: NixProgressEvent) {
        let AppMode::Update(UpdateState::Running { nix_progress, .. }) = &mut self.mode else {
            return;
        };

        match event {
            NixProgressEvent::Section { title } => {
                nix_progress.section = title;
            }
            NixProgressEvent::Activating => {
                nix_progress.section = "Activating new system generation".to_string();
                upsert_progress_row(
                    &mut nix_progress.rows,
                    NixProgressRow {
                        name: "system generation".to_string(),
                        status: NixProgressStatus::Activating,
                        transferred: None,
                        total: None,
                        speed_bps: None,
                        eta_secs: None,
                    },
                );
            }
            NixProgressEvent::Building { name } => {
                nix_progress.section = "Building derivations".to_string();
                upsert_progress_row(
                    &mut nix_progress.rows,
                    NixProgressRow {
                        name,
                        status: NixProgressStatus::Building,
                        transferred: None,
                        total: None,
                        speed_bps: None,
                        eta_secs: None,
                    },
                );
            }
            NixProgressEvent::Download {
                name,
                transferred,
                total,
                speed_bps,
                eta_secs,
            } => {
                nix_progress.section = "Downloading paths from cache".to_string();
                upsert_progress_row(
                    &mut nix_progress.rows,
                    NixProgressRow {
                        name,
                        status: NixProgressStatus::Downloading,
                        transferred: Some(transferred),
                        total,
                        speed_bps,
                        eta_secs,
                    },
                );
            }
            NixProgressEvent::Complete { name } => {
                if let Some(row) = nix_progress.rows.iter_mut().find(|row| row.name == name) {
                    row.status = NixProgressStatus::Complete;
                    if let Some(total) = row.total {
                        row.transferred = Some(total);
                    }
                    row.speed_bps = None;
                    row.eta_secs = None;
                }
            }
            NixProgressEvent::Failed { name } => {
                upsert_progress_row(
                    &mut nix_progress.rows,
                    NixProgressRow {
                        name,
                        status: NixProgressStatus::Failed,
                        transferred: None,
                        total: None,
                        speed_bps: None,
                        eta_secs: None,
                    },
                );
            }
        }

        while nix_progress.rows.len() > 12 {
            nix_progress.rows.remove(0);
        }
    }

    fn mark_step_skipped(&mut self, step_id: &str, reason: Option<&str>) {
        self.log_to_screen(&format!("[-] Step skipped: {}", step_id));
        if let Some(reason) = reason {
            self.log_to_screen(&format!("  {}", reason));
        }

        match &mut self.mode {
            AppMode::Update(UpdateState::Preparing { steps, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Skipped;
                    s.detail = reason.map(ToString::to_string);
                }
                if let Some(next_index) = steps
                    .iter()
                    .position(|step| step.id == step_id)
                    .and_then(|index| (index + 1 < steps.len()).then_some(index + 1))
                {
                    steps[next_index].status = StepState::Running;
                    steps[next_index].started_at = Some(Instant::now());
                }
            }
            AppMode::Update(UpdateState::Running { steps, step, .. }) => {
                if let Some(s) = Self::find_step_mut(steps, step_id) {
                    s.status = StepState::Skipped;
                    s.detail = reason.map(ToString::to_string);
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                    steps[*step].started_at = Some(Instant::now());
                }
            }
            _ => {}
        }
    }

    async fn handle_command_done(&mut self, success: bool) {
        self.log_to_screen(&format!(
            "\n=== Operation {} ===\n",
            if success { "COMPLETED" } else { "FAILED" }
        ));

        match &mut self.mode {
            AppMode::Apps(AppProfileState::Running { output, .. }) => {
                self.mode = AppMode::Apps(AppProfileState::Complete {
                    success,
                    output: output.clone(),
                    scroll_offset: None, // None = auto-scroll continues
                });
            }
            AppMode::Keys(KeysState::Running { output, .. }) => {
                self.mode = AppMode::Keys(KeysState::Complete {
                    success,
                    output: output.clone(),
                    scroll_offset: None, // None = auto-scroll continues
                });
            }
            AppMode::Install(InstallState::Running { output, .. }) => {
                self.mode = AppMode::Install(InstallState::Complete {
                    success,
                    output: output.clone(),
                    scroll_offset: None, // None = auto-scroll continues
                });
            }
            AppMode::Update(UpdateState::Running {
                steps,
                output,
                stash,
                ..
            }) => {
                let stash = stash.clone();
                let mut final_output = output.clone();

                // If we stashed changes and update succeeded, restore them
                if let Some(stash) = stash.clone().filter(|_| success) {
                    let config_path = nixos_config_dir();
                    let config_str = config_path.to_string_lossy().to_string();

                    final_output.push_back("".to_string());
                    final_output.push_back(format!(
                        "Restoring stashed changes from {}...",
                        stash.reference
                    ));

                    match run_capture(
                        "git",
                        &["-C", &config_str, "stash", "pop", &stash.reference],
                    )
                    .await
                    {
                        Ok((pop_ok, stdout, stderr)) => {
                            if pop_ok {
                                final_output.push_back(
                                    "  ✓ Stashed changes restored successfully".to_string(),
                                );
                            } else {
                                final_output
                                    .push_back("  ✗ Failed to restore stashed changes".to_string());
                                if !stderr.is_empty() {
                                    final_output.push_back(format!("    {}", stderr.trim()));
                                }
                                final_output.push_back(format!(
                                    "    Run 'git stash pop {}' manually to restore",
                                    stash.reference
                                ));
                            }
                            if !stdout.is_empty() {
                                for line in stdout.lines() {
                                    final_output.push_back(format!("    {}", line));
                                }
                            }
                        }
                        Err(e) => {
                            final_output.push_back(format!("  ✗ Error restoring stash: {}", e));
                            final_output.push_back(format!(
                                "    Run 'git stash pop {}' manually to restore",
                                stash.reference
                            ));
                        }
                    }
                }

                if let Some(stash) = stash.filter(|_| !success) {
                    final_output.push_back("".to_string());
                    final_output.push_back(format!(
                        "Autostash preserved at {} ({})",
                        stash.reference, stash.message
                    ));
                    final_output.push_back(format!(
                        "  Run 'git stash pop {}' after resolving the failure.",
                        stash.reference
                    ));
                }

                // Clear pending operation on completion
                if success {
                    super::resume::clear_pending();
                    self.pending_resume = None;
                }

                // Transition to ShowingSummary if we have a summary, otherwise go to Complete
                if let Some(summary) = self.update_summary.take() {
                    self.mode = AppMode::Update(UpdateState::ShowingSummary {
                        success,
                        steps: steps.clone(),
                        output: final_output,
                        summary,
                        scroll_offset: None,
                        summary_scroll: 0,
                    });
                } else {
                    self.mode = AppMode::Update(UpdateState::Complete {
                        success,
                        steps: steps.clone(),
                        output: final_output,
                        scroll_offset: None,
                    });
                }
            }
            AppMode::CreateHost(CreateHostState::Generating { config, .. }) => {
                self.mode = AppMode::CreateHost(CreateHostState::Complete {
                    success,
                    config: config.clone(),
                });
            }
            _ => {}
        }
    }

    fn handle_command_cancelled(&mut self) {
        self.log_to_screen("\n=== Operation CANCELLED ===\n");

        // Clear the cancellation token
        self.cancel_token = None;

        match &mut self.mode {
            AppMode::Update(UpdateState::Preparing {
                output, options, ..
            }) => {
                let options = options.clone();
                output.push_back("Operation cancelled by user.".to_string());
                self.mode = AppMode::Update(UpdateState::preflight(options, false));
            }
            AppMode::Apps(AppProfileState::Running { output, .. }) => {
                output.push_back("Operation cancelled by user.".to_string());
                self.mode = AppMode::Apps(AppProfileState::Complete {
                    success: false,
                    output: output.clone(),
                    scroll_offset: None,
                });
            }
            AppMode::Keys(KeysState::Running { output, .. }) => {
                output.push_back("Operation cancelled by user.".to_string());
                self.mode = AppMode::Keys(KeysState::Complete {
                    success: false,
                    output: output.clone(),
                    scroll_offset: None,
                });
            }
            AppMode::Install(InstallState::Running { output, .. }) => {
                output.push_back("Operation cancelled by user.".to_string());
                self.mode = AppMode::Install(InstallState::Complete {
                    success: false,
                    output: output.clone(),
                    scroll_offset: None,
                });
            }
            AppMode::Update(UpdateState::Running {
                steps,
                output,
                stash,
                ..
            }) => {
                let mut final_output = output.clone();
                final_output.push_back("Operation cancelled by user.".to_string());

                if let Some(stash) = stash.clone() {
                    final_output.push_back("".to_string());
                    final_output.push_back(format!(
                        "Autostash preserved at {} ({})",
                        stash.reference, stash.message
                    ));
                    final_output.push_back(format!(
                        "  Run 'git stash pop {}' after resolving the cancellation.",
                        stash.reference
                    ));
                }

                if let Some(summary) = self.update_summary.take() {
                    self.mode = AppMode::Update(UpdateState::ShowingSummary {
                        success: false,
                        steps: steps.clone(),
                        output: final_output,
                        summary,
                        scroll_offset: None,
                        summary_scroll: 0,
                    });
                } else {
                    self.mode = AppMode::Update(UpdateState::Complete {
                        success: false,
                        steps: steps.clone(),
                        output: final_output,
                        scroll_offset: None,
                    });
                }
            }
            AppMode::CreateHost(CreateHostState::Generating { config, output, .. }) => {
                output.push_back("Operation cancelled by user.".to_string());
                self.mode = AppMode::CreateHost(CreateHostState::Complete {
                    success: false,
                    config: config.clone(),
                });
            }
            _ => {}
        }
    }
}
