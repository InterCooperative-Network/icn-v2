#!/bin/bash
# -----------------------------------------------------------------------------
# setup_git_safety.sh: Configures Git environment for safety and productivity.
# Version: 3
#
# Features:
# - Adds Git branch display to Bash/Zsh prompts (gh-pages shown in red).
# - Sets useful global Git aliases (br, hist, save, unstage, st, co, sw, etc.).
# - Configures safer pull, fetch, and rebase defaults globally.
# - Installs a pre-commit hook in the current repository to prevent
#   accidental commits to 'main' and bypass checks on 'gh-pages'.
# - [Optional] Installs a pre-push hook to block 'WIP' commits.
# - Adds safety aliases like 'clean-safe' and 'safe-force-push'.
#
# Idempotent: Safe to run multiple times.
# Compatibility: Aims for macOS and Linux compatibility.
# Usage: ./scripts/setup_git_safety.sh [--install-pre-push-hook]
# Note: Shell config changes require restarting the shell or sourcing the file.
# -----------------------------------------------------------------------------
set -e # Exit immediately if a command exits with a non-zero status.

INSTALL_PRE_PUSH_HOOK=false
if [[ "$1" == "--install-pre-push-hook" ]]; then
    INSTALL_PRE_PUSH_HOOK=true
    echo "ℹ️ Pre-push hook installation requested."
fi

echo "🚀 Starting Git safety setup (v3)..."

# --- Shell Prompt Configuration ---
echo "🔧 Configuring shell prompts..."

BASHRC_FILE=~/.bashrc
ZSHRC_FILE=~/.zshrc
PROMPT_MARKER="# Added by setup_git_safety.sh" # Keep original marker for upgrade path

# Bash Prompt Function (with gh-pages in red)
BASH_PROMPT_FUNC='\n# Function to parse Git branch and color gh-pages red\nparse_git_branch() {\n  local branch=$(git symbolic-ref --short HEAD 2>/dev/null)\n  if [ -n "$branch" ]; then\n    if [ "$branch" = "gh-pages" ]; then\n      # Red color for gh-pages\n      printf " (\\[\\033[0;31m\\]%s\\[\\033[0m\\])" "$branch"\n    else\n      # Default green color for other branches\n      printf " (\\[\\033[0;32m\\]%s\\[\\033[0m\\])" "$branch"\n    fi\n  fi\n}\n'
# Customize your PS1 as needed, this integrates the function call
BASH_PS1_UPDATE='export PS1="\\u@\\h:\\w\\\$(parse_git_branch)\\\$ "'

# Zsh Prompt Setup (with gh-pages in red)
ZSH_PROMPT_SETUP='\n# Load version control information\nautoload -Uz vcs_info && precmd() { vcs_info }\n# Define formats, making gh-pages red\nzstyle ":vcs_info:git*" formats " (%F{green}%b%f)" # Default green\nzstyle ":vcs_info:git*:branch:gh-pages" formats " (%F{red}%b%f)" # Red for gh-pages\n# Ensure PROMPT includes ${vcs_info_msg_0_}, e.g.:\n# PROMPT="%n@%m:%~\\\${vcs_info_msg_0_}$ "\nsetopt PROMPT_SUBST # Ensure substitution happens\n'

# Function to add config block if marker not found
add_shell_config() {
    local rc_file="$1"
    local config_block="$2"
    local ps1_update_cmd="$3" # Optional: PS1 update specific to bash
    local shell_name="$4"

    if [ -f "$rc_file" ]; then
        # Check if our specific marker exists
        if ! grep -qF "$PROMPT_MARKER" "$rc_file"; then
             echo "  -> Adding Git branch prompt config to $rc_file"
             echo -e "\n$PROMPT_MARKER" >> "$rc_file"
             echo "$config_block" >> "$rc_file" # Add the function/setup
             if [ -n "$ps1_update_cmd" ]; then
                  echo "$ps1_update_cmd" >> "$rc_file" # Add the PS1 update for bash
                  echo "     (Note: Bash PS1 updated. Customize if needed.)"
             else
                  echo "     (Note: Zsh vcs_info added. Ensure PROMPT uses \${vcs_info_msg_0_})"
             fi
        else
             echo "  -> $shell_name prompt already configured in $rc_file (marker found)."
        fi
    else
        echo "  -> $rc_file not found, skipping $shell_name setup."
    fi
}

# Apply shell configs
add_shell_config "$BASHRC_FILE" "$BASH_PROMPT_FUNC" "$BASH_PS1_UPDATE" "Bash"
add_shell_config "$ZSHRC_FILE" "$ZSH_PROMPT_SETUP" "" "Zsh"

# --- Git Configuration (Global) ---
echo "🔧 Setting global Git configurations and aliases..."

# Function to set global config if not already set to the desired value
set_git_config() {
    local config_key="$1"
    local expected_value="$2"
    local current_value
    current_value=$(git config --global --get "$config_key" 2>/dev/null || echo "")

    if [ "$current_value" != "$expected_value" ]; then
        git config --global "$config_key" "$expected_value"
        echo "  -> Set Git config: $config_key = $expected_value"
    else
        echo "  -> Git config '$config_key' already set correctly."
    fi
}

# Function to set alias if not already set to the desired value
set_git_alias() {
    local alias_name="$1"
    local alias_command="$2"
    local current_value
    current_value=$(git config --global --get "alias.$alias_name" 2>/dev/null || echo "")

    # Normalize boolean representations for comparison if necessary
    # Example: alias.save might be set slightly differently but functionally same

    if [ "$current_value" != "$alias_command" ]; then
        git config --global "alias.$alias_name" "$alias_command"
        echo "  -> Set Git alias: $alias_name"
    else
        echo "  -> Git alias '$alias_name' already set correctly."
    fi
}

# 1. Auto-fetch Before Pull
set_git_config "pull.ff" "only"
set_git_config "fetch.prune" "true"

# 2. Enable Rebase Safety
set_git_config "rebase.autosquash" "true"
set_git_config "rebase.autoStash" "true"
set_git_config "rebase.abbreviateCommands" "true" # Minor convenience

# Core Aliases (from previous version)
set_git_alias "br" "rev-parse --abbrev-ref HEAD"
set_git_alias "hist" "log --graph --abbrev-commit --decorate --format=format:'%C(bold blue)%h%C(reset) - %C(bold green)(%ar)%C(reset) %C(white)%s%C(reset) %C(dim white)- %an%C(reset)%C(auto)%d%C(reset)' --all"
set_git_alias "save" '!git add -A && git commit -m "WIP"'
set_git_alias "unstage" "reset HEAD --"

# Optional useful aliases (from previous version)
set_git_alias "st" "status -sb"
set_git_alias "co" "checkout"
set_git_alias "sw" "switch"
set_git_alias "log" "log --oneline --graph --decorate"
set_git_alias "cm" "commit -m"
set_git_alias "ca" "commit --amend --no-edit"
set_git_alias "cfg" "config --list"

# 5. Git Clean Safety Alias
set_git_alias "clean-safe" '!git clean -nfd'

# 7. Git Safe Force Push Alias (handle complex quoting)
SAFE_FORCE_PUSH_COMMAND='!f() { branch=$(git rev-parse --abbrev-ref HEAD); if [[ $branch == "main" || $branch == "gh-pages" ]]; then echo "❌ Refusing to force push to protected branch: $branch"; exit 1; else echo "Attempting safe force push to $branch..."; git push --force-with-lease; fi }; f'
set_git_alias "safe-force-push" "$SAFE_FORCE_PUSH_COMMAND"


echo "  -> Global Git configurations and aliases checked/configured."


# --- Git Hooks Installation (Repo-Local) ---

# Function to install a hook script idempotently
install_hook() {
    local hook_name="$1" # e.g., "pre-commit"
    local hook_marker="$2" # e.g., "# HOOK_INSTALLED_BY_SETUP_SCRIPT_V2"
    local hook_content="$3"
    local install_flag="$4" # boolean indicating if installation is requested

    if ! git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
        echo "  -> Not inside a Git repository. Skipping $hook_name hook installation."
        return
    fi

    if [ "$install_flag" = false ] && [ "$hook_name" == "pre-push" ]; then
         echo "  -> Skipping optional pre-push hook installation (use --install-pre-push-hook flag to enable)."
         return
    fi

    local hook_dir
    hook_dir=$(git rev-parse --git-path hooks) # Reliable way to find hooks dir
    local hook_file="$hook_dir/$hook_name"

    # Create hook directory if it doesn't exist
    mkdir -p "$hook_dir"

    local should_install=false
    if [ ! -f "$hook_file" ]; then
        should_install=true
        echo "  -> $hook_name hook file does not exist. Installing."
    elif ! grep -qF "$hook_marker" "$hook_file"; then
         should_install=true
         echo "  -> $hook_name hook file exists but lacks correct marker. Overwriting."
    fi

    if [ "$should_install" = true ]; then
        echo "  -> Installing/updating $hook_name hook in $hook_file"
        echo -e "$hook_content" > "$hook_file" # Use -e to interpret escapes like \

        chmod +x "$hook_file"
        echo "  -> Hook installed/updated and made executable."
    else
        echo "  -> $hook_name hook already exists and has the correct marker. Skipping installation."
    fi
}

# --- Pre-commit Hook ---
echo "🔧 Installing pre-commit hook..."
PRE_COMMIT_MARKER="# HOOK_INSTALLED_BY_SETUP_SCRIPT_V2"
PRE_COMMIT_CONTENT='#!/bin/bash
'"$PRE_COMMIT_MARKER"'
# Pre-commit hook:
# - Prevents accidental commits to the main branch without confirmation.
# - Allows commits on the gh-pages branch without warning.

PROTECTED_BRANCH="main"
ALLOWED_BRANCH="gh-pages"
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

# Allow commits on gh-pages without interaction
if [ "$CURRENT_BRANCH" = "$ALLOWED_BRANCH" ]; then
  # echo "Committing to $ALLOWED_BRANCH branch - pre-commit hook bypassed." # Optional: uncomment for verbosity
  exit 0
# Warn and require confirmation for main branch
elif [ "$CURRENT_BRANCH" = "$PROTECTED_BRANCH" ]; then
  # Check if running interactively
  if [ -t 1 ]; then
      # Simplified prompt string for read -p
      read -p "WARNING: Committing directly to '$PROTECTED_BRANCH'. Continue? (y/N) " -n 1 -r
      echo # Move to a new line
      if [[ ! "$REPLY" =~ ^[Yy]$ ]]; then
        echo "Commit aborted by user."
        exit 1
      fi
  else
      # Non-interactive environment (e.g., CI/CD), fail safe
      echo "ERROR: Attempted commit to '$PROTECTED_BRANCH' branch in non-interactive environment."
      echo "Commit aborted."
      exit 1
  fi
fi

# Allow commits on other branches
exit 0'
install_hook "pre-commit" "$PRE_COMMIT_MARKER" "$PRE_COMMIT_CONTENT" true # Always install this one


# --- Pre-push Hook (Optional) ---
echo "🔧 Checking pre-push hook status..."
PRE_PUSH_MARKER="# HOOK_INSTALLED_BY_SETUP_SCRIPT_V1_PREPUSH"
PRE_PUSH_CONTENT='#!/bin/bash\n'"$PRE_PUSH_MARKER"'\n# Pre-push hook:\n# - Blocks pushing commits with the message "WIP".\n# Add other checks here if needed (e.g., large files, secrets).\n\n# Check for WIP commits\nif git log origin/$(git rev-parse --abbrev-ref HEAD)..HEAD --pretty=%B | grep -Eq "^WIP$"; then\n  echo "❌ Refusing to push: Found commit(s) with message \\\"WIP\\\" in the push range."\n  echo "   Please amend the commit message(s) before pushing."\n  exit 1\nfi\n\n# Add checks for large files (example using git-lfs, adapt if needed)\n# git lfs pre-push "$@"\n# if [ $? -ne 0 ]; then exit 1; fi\n\n# Add checks for secrets (example using gitleaks, adapt if needed)\n# gitleaks protect --verbose -c /path/to/.gitleaks.toml\n# if [ $? -ne 0 ]; then exit 1; fi\n\necho "✅ Pre-push checks passed."\nexit 0\n'
# Install pre-push hook only if flag is passed
install_hook "pre-push" "$PRE_PUSH_MARKER" "$PRE_PUSH_CONTENT" "$INSTALL_PRE_PUSH_HOOK"


# --- Final Check ---
echo "🔍 Performing final branch check..."
if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    CURRENT_BRANCH_FINAL=$(git rev-parse --abbrev-ref HEAD)
    if [ "$CURRENT_BRANCH_FINAL" = "main" ]; then
        echo "⚠️ Currently on the '$CURRENT_BRANCH_FINAL' branch. Commits require confirmation (pre-commit hook)."
    elif [ "$CURRENT_BRANCH_FINAL" = "gh-pages" ]; then
        echo "✅ Currently on the '$CURRENT_BRANCH_FINAL' branch (hooks are bypassed)."
    else
        echo "👍 Currently on feature branch '$CURRENT_BRANCH_FINAL'."
    fi
else
    echo "  -> Not in a git repo, skipping branch check."
fi

echo "✅ Git safety setup script finished!"
echo "ℹ️ Remember to restart your shell or run 'source ~/.bashrc' / 'source ~/.zshrc' for prompt changes to take effect."
echo "ℹ️ Consider creating '.editorconfig' and '.github/workflows/prevent-main-commits.yml' (if using GitHub) for team consistency and protection." 