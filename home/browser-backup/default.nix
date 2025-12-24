# Browser Profile Backup/Restore Module
#
# Provides age-encrypted browser profile backup to a private GitHub repository.
# Supports 1Password integration for automatic key retrieval across machines.
#
# MINIMAL BACKUP - Only backs up essential files for:
# - Login sessions and cookies
# - Saved passwords (encrypted by browser)
# - Firefox Sync / Chrome sync data
# - Current session tabs
#
# Does NOT backup: cache, history, extensions, themes, or other data.
# Typical backup size: <10MB (vs 3GB+ for full profiles)
#
# See CLAUDE.md for setup instructions and troubleshooting.
#
{ config, pkgs, lib, ... }:

with lib;

let
  cfg = config.programs.browser-backup;

  # Essential Chrome files for login/session restoration
  # These are the minimum files needed to preserve:
  # - Cookies (session cookies for logged-in sites)
  # - Login Data (saved passwords, encrypted)
  # - Web Data (autofill, form data)
  # - Session data (open tabs)
  # - Preferences and settings
  chromeEssentialFiles = [
    "Default/Cookies"
    "Default/Cookies-journal"
    "Default/Login Data"
    "Default/Login Data-journal"
    "Default/Web Data"
    "Default/Web Data-journal"
    "Default/Preferences"
    "Default/Secure Preferences"
    "Default/Current Session"
    "Default/Current Tabs"
    "Default/Last Session"
    "Default/Last Tabs"
    "Default/Bookmarks"
    "Default/Favicons"
    "Default/Favicons-journal"
    "Local State"
  ];

  # Essential Firefox files for login/session restoration
  # Firefox stores profile data in random-named subdirectories
  # These glob patterns match the essential files in any profile
  firefoxEssentialPatterns = [
    "*.default*/cookies.sqlite"
    "*.default*/cookies.sqlite-wal"
    "*.default*/logins.json"
    "*.default*/key4.db"
    "*.default*/cert9.db"
    "*.default*/prefs.js"
    "*.default*/sessionstore.jsonlz4"
    "*.default*/sessionstore-backups/recovery.jsonlz4"
    "*.default*/signons.sqlite"
    "*.default*/formhistory.sqlite"
    "*.default*/places.sqlite"
    "*.default*/favicons.sqlite"
    "profiles.ini"
    "installs.ini"
  ];

  # The browser-backup script
  browser-backup = pkgs.writeShellApplication {
    name = "browser-backup";
    runtimeInputs = with pkgs; [ coreutils gnutar gzip age git git-lfs _1password-cli findutils ];
    text = ''
      set -euo pipefail

      # Colors
      RED='\033[0;31m'
      GREEN='\033[0;32m'
      YELLOW='\033[1;33m'
      BLUE='\033[0;34m'
      NC='\033[0m'

      log_info() { echo -e "''${BLUE}[INFO]''${NC} $1"; }
      log_success() { echo -e "''${GREEN}[SUCCESS]''${NC} $1"; }
      log_warn() { echo -e "''${YELLOW}[WARN]''${NC} $1"; }
      log_error() { echo -e "''${RED}[ERROR]''${NC} $1"; exit 1; }

      # Load configuration
      CONFIG_FILE="$HOME/.config/browser-backup/config"
      if [[ ! -f "$CONFIG_FILE" ]]; then
        log_error "Config file not found: $CONFIG_FILE"
      fi
      # shellcheck source=/dev/null
      source "$CONFIG_FILE"

      # Validate required config
      : "''${BROWSER_BACKUP_REPO:?BROWSER_BACKUP_REPO not set in config}"
      : "''${AGE_RECIPIENT:?AGE_RECIPIENT not set in config}"
      : "''${LOCAL_REPO_PATH:=$HOME/.local/share/browser-backup}"

      # Parse arguments
      FORCE=false
      PUSH=false
      while [[ $# -gt 0 ]]; do
        case $1 in
          --force|-f) FORCE=true; shift ;;
          --push|-p) PUSH=true; shift ;;
          --help|-h)
            echo "Usage: browser-backup [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --force, -f    Force backup even if browsers are running"
            echo "  --push, -p     Push encrypted archives to GitHub after backup"
            echo "  --help, -h     Show this help"
            echo ""
            echo "This backs up ONLY essential files for login restoration:"
            echo "  Chrome: cookies, login data, sessions, preferences"
            echo "  Firefox: cookies, logins, sessions, sync data"
            exit 0
            ;;
          *) log_error "Unknown option: $1" ;;
        esac
      done

      # Check if browsers are running
      check_browsers() {
        local running=""
        pgrep -x "chrome" >/dev/null 2>&1 && running="Chrome"
        pgrep -x "firefox" >/dev/null 2>&1 && running="''${running:+$running, }Firefox"
        pgrep -x "firefox-bin" >/dev/null 2>&1 && running="''${running:+$running, }Firefox"
        pgrep -x ".firefox-wrappe" >/dev/null 2>&1 && running="''${running:+$running, }Firefox"
        if [[ -n "$running" ]]; then
          if [[ "$FORCE" == "true" ]]; then
            log_warn "Browsers running ($running) - continuing with --force"
          else
            log_error "Browsers are running: $running. Close them or use --force"
          fi
        fi
      }

      # Create Chrome archive with only essential files
      backup_chrome() {
        local chrome_dir="$HOME/.config/google-chrome"
        local archive="$TEMP_DIR/chrome-profile.tar.gz"
        local filelist="$TEMP_DIR/chrome-files.txt"

        if [[ ! -d "$chrome_dir" ]]; then
          log_warn "Chrome directory not found: $chrome_dir"
          return 1
        fi

        log_info "Backing up Chrome essential files..."

        # Build list of files that exist
        : > "$filelist"
        cd "$chrome_dir"
        for f in ${lib.concatMapStringsSep " " (f: ''"${f}"'') chromeEssentialFiles}; do
          if [[ -f "$f" ]]; then
            echo "$f" >> "$filelist"
          fi
        done

        if [[ ! -s "$filelist" ]]; then
          log_warn "No Chrome files found to backup"
          return 1
        fi

        local count
        count=$(wc -l < "$filelist")
        log_info "Found $count essential Chrome files"

        # Create archive from file list
        tar --create --gzip --file="$archive" \
          --directory="$chrome_dir" \
          --files-from="$filelist" \
          --sort=name \
          --mtime='2024-01-01'

        local size
        size=$(du -h "$archive" | cut -f1)
        log_success "Chrome archive: $size"
        return 0
      }

      # Create Firefox archive with only essential files
      backup_firefox() {
        local firefox_dir="$HOME/.mozilla/firefox"
        local archive="$TEMP_DIR/firefox-profile.tar.gz"
        local filelist="$TEMP_DIR/firefox-files.txt"

        if [[ ! -d "$firefox_dir" ]]; then
          log_warn "Firefox directory not found: $firefox_dir"
          return 1
        fi

        log_info "Backing up Firefox essential files..."

        # Find files matching essential patterns
        cd "$firefox_dir"
        : > "$filelist"
        for pattern in ${lib.concatMapStringsSep " " (p: ''"${p}"'') firefoxEssentialPatterns}; do
          # Use find with -path for glob matching
          find . -path "./$pattern" -type f 2>/dev/null | sed 's|^\./||' >> "$filelist" || true
        done

        if [[ ! -s "$filelist" ]]; then
          log_warn "No Firefox files found to backup"
          return 1
        fi

        local count
        count=$(wc -l < "$filelist")
        log_info "Found $count essential Firefox files"

        # Create archive from file list
        tar --create --gzip --file="$archive" \
          --directory="$firefox_dir" \
          --files-from="$filelist" \
          --sort=name \
          --mtime='2024-01-01'

        local size
        size=$(du -h "$archive" | cut -f1)
        log_success "Firefox archive: $size"
        return 0
      }

      # Encrypt with age (only needs public key - no 1Password needed)
      encrypt_archive() {
        local src="$1" dst="$2"
        log_info "Encrypting: $(basename "$src")"
        age --encrypt --recipient "$AGE_RECIPIENT" --output "$dst" "$src"
        # Securely remove unencrypted archive
        shred -u "$src" 2>/dev/null || rm -f "$src"
        local size
        size=$(du -h "$dst" | cut -f1)
        log_success "Encrypted: $(basename "$dst") ($size)"
      }

      # Check if LFS is needed and set up
      setup_lfs_if_needed() {
        local file="$1"
        local size
        size=$(stat -c%s "$file" 2>/dev/null || echo 0)
        if [[ $size -gt 104857600 ]]; then  # 100MB
          log_info "File exceeds 100MB, setting up Git LFS..."
          if ! git lfs env &>/dev/null; then
            git lfs install
          fi
          if ! grep -qF '*.age filter=lfs' .gitattributes 2>/dev/null; then
            echo '*.age filter=lfs diff=lfs merge=lfs -text' >> .gitattributes
            git add .gitattributes
            log_success "Git LFS configured for .age files"
          fi
        fi
      }

      # Main
      log_info "Browser Profile Backup (Essential Files Only)"
      echo ""

      check_browsers

      # Create secure temp directory
      TEMP_DIR=$(mktemp -d)
      chmod 700 "$TEMP_DIR"
      trap 'rm -rf "$TEMP_DIR"' EXIT INT TERM

      # Chrome backup
      if backup_chrome; then
        encrypt_archive "$TEMP_DIR/chrome-profile.tar.gz" "$TEMP_DIR/chrome-profile.tar.gz.age"
      fi

      # Firefox backup
      if backup_firefox; then
        encrypt_archive "$TEMP_DIR/firefox-profile.tar.gz" "$TEMP_DIR/firefox-profile.tar.gz.age"
      fi

      # Push to GitHub if requested
      if [[ "$PUSH" == "true" ]]; then
        echo ""
        log_info "Pushing to GitHub..."

        # Clone or update repo
        LOCAL_REPO_PATH="''${LOCAL_REPO_PATH/#\~/$HOME}"
        mkdir -p "$(dirname "$LOCAL_REPO_PATH")"
        if [[ ! -d "$LOCAL_REPO_PATH/.git" ]]; then
          log_info "Cloning repository..."
          git clone "$BROWSER_BACKUP_REPO" "$LOCAL_REPO_PATH"
        else
          log_info "Updating repository..."
          git -C "$LOCAL_REPO_PATH" pull --rebase
        fi

        # Copy encrypted files
        cd "$LOCAL_REPO_PATH"
        for age_file in "$TEMP_DIR"/*.age; do
          if [[ -f "$age_file" ]]; then
            cp "$age_file" .
            setup_lfs_if_needed "$(basename "$age_file")"
          fi
        done

        # Commit and push
        git add -A
        if git diff --staged --quiet; then
          log_info "No changes to commit"
        else
          git commit -m "Backup $(date +%Y-%m-%d\ %H:%M)"
          git push
          log_success "Pushed to GitHub"
        fi
      fi

      echo ""
      log_success "Backup complete!"
      if [[ "$PUSH" != "true" ]]; then
        log_info "Encrypted files in: $TEMP_DIR"
        log_info "Use --push to upload to GitHub"
        # Don't clean up temp dir if user might want the files
        trap - EXIT INT TERM
      fi
    '';
  };

  # The browser-restore script
  browser-restore = pkgs.writeShellApplication {
    name = "browser-restore";
    runtimeInputs = with pkgs; [ coreutils gnutar gzip age git git-lfs _1password-cli ];
    text = ''
      set -euo pipefail

      # Colors
      RED='\033[0;31m'
      GREEN='\033[0;32m'
      YELLOW='\033[1;33m'
      BLUE='\033[0;34m'
      NC='\033[0m'

      log_info() { echo -e "''${BLUE}[INFO]''${NC} $1"; }
      log_success() { echo -e "''${GREEN}[SUCCESS]''${NC} $1"; }
      log_warn() { echo -e "''${YELLOW}[WARN]''${NC} $1"; }
      log_error() { echo -e "''${RED}[ERROR]''${NC} $1"; exit 1; }

      # Load configuration
      CONFIG_FILE="$HOME/.config/browser-backup/config"
      if [[ ! -f "$CONFIG_FILE" ]]; then
        log_error "Config file not found: $CONFIG_FILE"
      fi
      # shellcheck source=/dev/null
      source "$CONFIG_FILE"

      # Validate required config
      : "''${BROWSER_BACKUP_REPO:?BROWSER_BACKUP_REPO not set in config}"
      : "''${LOCAL_REPO_PATH:=$HOME/.local/share/browser-backup}"
      : "''${BACKUP_RETENTION:=3}"

      # Check for 1Password or file-based key
      USE_1PASSWORD=false
      if [[ -n "''${AGE_KEY_1PASSWORD:-}" ]]; then
        USE_1PASSWORD=true
      elif [[ -z "''${AGE_KEY_PATH:-}" ]]; then
        log_error "Neither AGE_KEY_1PASSWORD nor AGE_KEY_PATH is set in config"
      fi

      # Parse arguments
      FORCE=false
      PULL=false
      while [[ $# -gt 0 ]]; do
        case $1 in
          --force|-f) FORCE=true; shift ;;
          --pull|-p) PULL=true; shift ;;
          --help|-h)
            echo "Usage: browser-restore [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --force, -f    Force restore even if browsers are running"
            echo "  --pull, -p     Pull latest from GitHub before restoring"
            echo "  --help, -h     Show this help"
            echo ""
            echo "This restores essential files for login restoration:"
            echo "  Chrome: cookies, login data, sessions, preferences"
            echo "  Firefox: cookies, logins, sessions, sync data"
            echo ""
            echo "Files are merged into existing browser profiles."
            exit 0
            ;;
          *) log_error "Unknown option: $1" ;;
        esac
      done

      # Check if browsers are running
      check_browsers() {
        local running=""
        pgrep -x "chrome" >/dev/null 2>&1 && running="Chrome"
        pgrep -x "firefox" >/dev/null 2>&1 && running="''${running:+$running, }Firefox"
        pgrep -x "firefox-bin" >/dev/null 2>&1 && running="''${running:+$running, }Firefox"
        pgrep -x ".firefox-wrappe" >/dev/null 2>&1 && running="''${running:+$running, }Firefox"
        if [[ -n "$running" ]]; then
          if [[ "$FORCE" == "true" ]]; then
            log_warn "Browsers running ($running) - continuing with --force"
          else
            log_error "Browsers are running: $running. Close them or use --force"
          fi
        fi
      }

      # Get age key (from 1Password or file)
      get_age_key() {
        if [[ "$USE_1PASSWORD" == "true" ]]; then
          # Retrieve from 1Password - this will prompt for unlock if needed
          log_info "Retrieving age key from 1Password..."
          op read "$AGE_KEY_1PASSWORD"
        else
          # Read from file
          local key_path="''${AGE_KEY_PATH/#\~/$HOME}"
          if [[ ! -f "$key_path" ]]; then
            log_error "Age identity key not found: $key_path"
          fi
          cat "$key_path"
        fi
      }

      # Backup essential files before overwriting
      backup_essential_files() {
        local target_dir="$1" archive_content_dir="$2"
        local timestamp
        timestamp=$(date +%Y%m%d-%H%M%S)
        local backup_dir="$target_dir.backup-essential.$timestamp"

        if [[ ! -d "$target_dir" ]]; then
          return 0
        fi

        # Only backup files that will be overwritten
        mkdir -p "$backup_dir"
        cd "$archive_content_dir"
        find . -type f | while read -r f; do
          local src="$target_dir/$f"
          local dst="$backup_dir/$f"
          if [[ -f "$src" ]]; then
            mkdir -p "$(dirname "$dst")"
            cp "$src" "$dst"
          fi
        done

        local count
        count=$(find "$backup_dir" -type f 2>/dev/null | wc -l)
        if [[ "$count" -gt 0 ]]; then
          log_info "Backed up $count existing files to: $(basename "$backup_dir")"
        else
          rm -rf "$backup_dir"
        fi
      }

      # Prune old essential backups
      prune_essential_backups() {
        local dir="$1" keep="$2"
        local parent
        parent=$(dirname "$dir")
        local base
        base=$(basename "$dir")

        # Find and delete oldest backups beyond retention
        # shellcheck disable=SC2012
        ls -dt "$parent/$base".backup-essential.* 2>/dev/null | tail -n +$((keep + 1)) | while read -r old_backup; do
          log_info "Pruning old backup: $(basename "$old_backup")"
          rm -rf "$old_backup"
        done
      }

      # Restore Chrome essential files (merge into existing profile)
      restore_chrome() {
        local age_file="$1"
        local chrome_dir="$HOME/.config/google-chrome"

        if [[ ! -f "$age_file" ]]; then
          log_warn "Chrome backup not found: $age_file"
          return 1
        fi

        log_info "Restoring Chrome essential files..."

        # Decrypt
        local tar_file="$TEMP_DIR/chrome-profile.tar.gz"
        local extract_dir="$TEMP_DIR/chrome-extract"
        mkdir -p "$extract_dir"

        get_age_key | age --decrypt --identity - --output "$tar_file" "$age_file"
        tar --extract --gzip --file="$tar_file" --directory="$extract_dir"

        # Backup files that will be overwritten
        backup_essential_files "$chrome_dir" "$extract_dir"
        prune_essential_backups "$chrome_dir" "$BACKUP_RETENTION"

        # Ensure target directory exists
        mkdir -p "$chrome_dir/Default"

        # Copy files from archive to chrome dir (merge)
        local file_count=0
        cd "$extract_dir"
        find . -type f | while read -r f; do
          local src="$extract_dir/$f"
          local dst="$chrome_dir/$f"
          mkdir -p "$(dirname "$dst")"
          cp "$src" "$dst"
        done
        file_count=$(find "$extract_dir" -type f 2>/dev/null | wc -l)
        cd /

        # Cleanup
        shred -u "$tar_file" 2>/dev/null || rm -f "$tar_file"
        rm -rf "$extract_dir"

        log_success "Restored $file_count Chrome essential files"
        return 0
      }

      # Restore Firefox essential files (merge into existing profile)
      restore_firefox() {
        local age_file="$1"
        local firefox_dir="$HOME/.mozilla/firefox"

        if [[ ! -f "$age_file" ]]; then
          log_warn "Firefox backup not found: $age_file"
          return 1
        fi

        log_info "Restoring Firefox essential files..."

        # Decrypt
        local tar_file="$TEMP_DIR/firefox-profile.tar.gz"
        local extract_dir="$TEMP_DIR/firefox-extract"
        mkdir -p "$extract_dir"

        get_age_key | age --decrypt --identity - --output "$tar_file" "$age_file"
        tar --extract --gzip --file="$tar_file" --directory="$extract_dir"

        # Backup files that will be overwritten
        backup_essential_files "$firefox_dir" "$extract_dir"
        prune_essential_backups "$firefox_dir" "$BACKUP_RETENTION"

        # Ensure target directory exists
        mkdir -p "$firefox_dir"

        # Copy files from archive to firefox dir (merge)
        local file_count=0
        cd "$extract_dir"
        find . -type f | while read -r f; do
          local src="$extract_dir/$f"
          local dst="$firefox_dir/$f"
          mkdir -p "$(dirname "$dst")"
          cp "$src" "$dst"
        done
        file_count=$(find "$extract_dir" -type f 2>/dev/null | wc -l)
        cd /

        # Cleanup
        shred -u "$tar_file" 2>/dev/null || rm -f "$tar_file"
        rm -rf "$extract_dir"

        log_success "Restored $file_count Firefox essential files"
        return 0
      }

      # Main
      log_info "Browser Profile Restore (Essential Files Only)"
      echo ""

      check_browsers

      # Pull from GitHub if requested
      LOCAL_REPO_PATH="''${LOCAL_REPO_PATH/#\~/$HOME}"
      if [[ "$PULL" == "true" ]]; then
        log_info "Pulling from GitHub..."
        mkdir -p "$(dirname "$LOCAL_REPO_PATH")"
        if [[ ! -d "$LOCAL_REPO_PATH/.git" ]]; then
          log_info "Cloning repository..."
          git clone "$BROWSER_BACKUP_REPO" "$LOCAL_REPO_PATH"
        else
          log_info "Updating repository..."
          git -C "$LOCAL_REPO_PATH" pull --rebase
        fi
        echo ""
      fi

      if [[ ! -d "$LOCAL_REPO_PATH" ]]; then
        log_error "Local repo not found: $LOCAL_REPO_PATH. Use --pull to clone."
      fi

      # Create secure temp directory
      TEMP_DIR=$(mktemp -d)
      chmod 700 "$TEMP_DIR"
      trap 'rm -rf "$TEMP_DIR"' EXIT INT TERM

      # Restore Chrome
      restore_chrome "$LOCAL_REPO_PATH/chrome-profile.tar.gz.age" || true

      # Restore Firefox
      restore_firefox "$LOCAL_REPO_PATH/firefox-profile.tar.gz.age" || true

      echo ""
      log_success "Restore complete!"
      log_info "You can now start your browsers."
    '';
  };

in
{
  options.programs.browser-backup = {
    enable = mkEnableOption "browser profile backup/restore";

    repoUrl = mkOption {
      type = types.str;
      default = "git@github.com:DigitalPals/private-settings.git";
      description = "Private GitHub repo URL for encrypted backups";
    };

    ageRecipient = mkOption {
      type = types.str;
      description = "Age public key for encryption";
      example = "age1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
    };

    ageKey1Password = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = ''
        1Password secret reference for the age private key.
        Format: op://vault/item/field
        Example: op://Private/age-key/private-key

        When set, the key is retrieved from 1Password on-the-fly.
        This takes precedence over ageKeyPath.
      '';
      example = "op://Private/age-key/private-key";
    };

    ageKeyPath = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = ''
        Path to age identity key file (fallback if ageKey1Password not set).
        Not recommended - prefer 1Password integration.
      '';
      example = "~/.config/age/key.txt";
    };

    localRepoPath = mkOption {
      type = types.str;
      default = "~/.local/share/browser-backup";
      description = "Local clone location for the backup repo";
    };

    backupRetention = mkOption {
      type = types.int;
      default = 3;
      description = "Number of timestamped backups to keep when restoring";
    };
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.ageKey1Password != null || cfg.ageKeyPath != null;
        message = "browser-backup: Either ageKey1Password or ageKeyPath must be set";
      }
    ];

    # Install the scripts and dependencies
    home.packages = [
      browser-backup
      browser-restore
      pkgs.age
      pkgs.git
      pkgs.git-lfs
      pkgs._1password-cli
    ];

    # Generate the configuration file
    xdg.configFile."browser-backup/config" = {
      text = ''
        # Browser Backup Configuration
        # Generated by Home Manager - do not edit manually
        BROWSER_BACKUP_REPO="${cfg.repoUrl}"
        AGE_RECIPIENT="${cfg.ageRecipient}"
        LOCAL_REPO_PATH="${cfg.localRepoPath}"
        BACKUP_RETENTION=${toString cfg.backupRetention}
      '' + optionalString (cfg.ageKey1Password != null) ''
        AGE_KEY_1PASSWORD="${cfg.ageKey1Password}"
      '' + optionalString (cfg.ageKeyPath != null && cfg.ageKey1Password == null) ''
        AGE_KEY_PATH="${cfg.ageKeyPath}"
      '';
      force = true;
    };
  };
}
