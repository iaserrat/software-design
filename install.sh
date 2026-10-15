#!/bin/sh
{
# Parse the whole script before running it, including when piped from curl.
set -eu

fail() {
  echo "software-design: $*" >&2
  exit 1
}

verify() {
  launcher="$1/scripts/software-design"
  [ -f "$launcher" ] || fail "Cannot find $launcher. Agent install paths may have changed."
  sh "$launcher" install || fail "Build failed. Fix the reported problem and rerun this installer."
  sh "$launcher" --version
  output=$(mktemp)
  sh "$launcher" mcp >"$output" <<'EOF' &
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"installer","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
EOF
  server=$!
  # Bound the check even if the server cannot finish its handshake.
  (sleep 15; kill "$server" 2>/dev/null || true) >/dev/null 2>&1 &
  watchdog=$!
  status=0
  wait "$server" || status=$?
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  if [ "$status" -ne 0 ] ||
    ! grep -Eq '"name"[[:space:]]*:[[:space:]]*"check"' "$output" ||
    ! grep -Eq '"name"[[:space:]]*:[[:space:]]*"interface"' "$output"; then
    cat "$output" >&2
    rm -f "$output"
    fail "MCP readiness check failed. Rerun the installer after fixing the server error."
  fi
  rm -f "$output"
}

main() {
  case "${1-}" in
    --help|-h)
      echo "Usage: sh install.sh [claude codex pi]"
      echo "Without arguments, offers the agents found on PATH. Builds before reporting success."
      return
      ;;
  esac
  case "$(uname -s)" in
    Darwin|Linux) ;;
    *) fail "Use macOS, Linux, or WSL." ;;
  esac

  agents="$*"
  if [ "$#" -eq 0 ]; then
    agents=""
    for agent in claude codex pi; do
      if command -v "$agent" >/dev/null 2>&1; then
        [ -r /dev/tty ] && [ -w /dev/tty ] || fail "No terminal. Pass the agents explicitly: sh install.sh codex"
        printf 'Install Software Design for %s? [Y/n] ' "$agent" >/dev/tty
        IFS= read -r answer </dev/tty || fail "Selection cancelled."
        case "$answer" in
          ''|y|Y|yes|YES) agents="$agents $agent" ;;
          n|N|no|NO) ;;
          *) fail "Expected yes or no. Rerun to select agents." ;;
        esac
      fi
    done
  fi
  [ -n "$agents" ] || fail "No agents selected; nothing installed. Install an agent CLI if none was found on PATH."
  for agent in $agents; do
    case "$agent" in
      claude|codex|pi) ;;
      *) fail "Unknown agent: $agent. Choose claude, codex, or pi." ;;
    esac
    command -v "$agent" >/dev/null 2>&1 || fail "$agent is not on PATH."
  done
  command -v cargo >/dev/null 2>&1 || fail "Install Rust from https://rustup.rs, then rerun."
  command -v git >/dev/null 2>&1 || fail "Install Git, then rerun."
  command -v "${CC:-cc}" >/dev/null 2>&1 || fail "Install a C compiler (macOS: xcode-select --install; Debian/Ubuntu: sudo apt install build-essential)."

  echo "Installing for:$agents. Close existing agent sessions before starting a new one."
  for agent in $agents; do
    echo "Preparing $agent..."
    case "$agent" in
      claude)
        source="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/plugins/marketplaces/software-design"
        if [ -d "$source" ]; then
          claude plugin marketplace update software-design
        else
          claude plugin marketplace add iaserrat/software-design
        fi
        verify "$source"
        claude plugin install software-design@software-design --scope user
        claude plugin update software-design@software-design --scope user
        ;;
      codex)
        source="${CODEX_HOME:-$HOME/.codex}/.tmp/marketplaces/software-design"
        if [ -d "$source" ]; then
          codex plugin marketplace upgrade software-design
        else
          codex plugin marketplace add iaserrat/software-design
        fi
        verify "$source"
        codex plugin add software-design@software-design
        ;;
      pi)
        pi install git:github.com/iaserrat/software-design
        source="${PI_CODING_AGENT_DIR:-$HOME/.pi/agent}/git/github.com/iaserrat/software-design"
        verify "$source"
        ;;
    esac
    echo "$agent: installed; binary and MCP tools verified."
  done
  echo "Installation complete. Start new sessions in:$agents"
}

main "$@"
}
