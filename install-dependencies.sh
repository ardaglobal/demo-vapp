#!/bin/bash

set -e
set -o pipefail

OS=$(uname -s)

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
TMPDIR=${TMPDIR:-/tmp}
ROOT="$DIR/"
CARGO_CRATES=cargo-nextest

# ==========//=========//=========//=========//==========//==========//==========
# general functions
say() {
  printf ":o %s\n" "$1"
}

warn() {
  say "warning: ${1}" >&2
}

err() {
  say "$1" >&2
  exit 1
}

check_cmd() {
  command -v "$1" &>/dev/null
}

need_cmd() {
  if ! check_cmd "$1"; then
    err "need '$1' (command not found)"
  fi
}

ensure() {
  if ! "$@"; then err "command failed: $*"; fi
}

function try_sudo() {
  if [ "$(id -u)" -eq 0 ]; then
    bash -c "$1"
  else
    sudo bash -c "$1"
  fi
}

function box_out() {
  local s=("$@") b w
  for l in "${s[@]}"; do
    ((w < ${#l})) && {
      b="$l"
      w="${#l}"
    }
  done
  tput setaf 3
  echo " -${b//?/-}-
| ${b//?/ } |"
  for l in "${s[@]}"; do
    printf '| %s%*s%s |\n' "$(tput setaf 4)" "-$w" "$l" "$(tput setaf 3)"
  done
  echo "| ${b//?/ } |
 -${b//?/-}-"
  tput sgr 0
}

function get_profile_path() {
  local PROFILE=
  local PREF_SHELL
  case $SHELL in
  */zsh)
    PROFILE=${ZDOTDIR-"$HOME"}/.zshenv
    ;;
  */bash)
    PROFILE=$HOME/.bash_profile
    ;;
  */fish)
    PROFILE=$HOME/.config/fish/config.fish
    ;;
  */ash)
    PROFILE=$HOME/.profile
    ;;
  *)
    echo ": could not detect shell"
    exit 1
    ;;
  esac

  echo $PROFILE
}

function get_shell_name() {
  local PREF_SHELL=
  case $SHELL in
  */zsh)
    PREF_SHELL=zsh
    ;;
  */bash)
    PREF_SHELL=bash
    ;;
  */fish)
    PREF_SHELL=fish
    ;;
  */ash)
    PREF_SHELL=ash
    ;;
  *)
    echo ": could not detect shell"
    exit 1
    ;;
  esac

  echo $PREF_SHELL
}

function add_path_to_profile() {
  local PROFILE="$1"
  local BIN_DIR="$2"

  # use a subshell to avoid clobbering the calling shell's $PATH
  if ! env -i HOME=$HOME TERM=$TERM $SHELL -c "export PATH=#:\$PATH; . \"$PROFILE\"; echo \$PATH | sed 's/:#.*//g' | grep -q \"${BIN_DIR}\""; then
    # Add the foundryup directory to the path and ensure the old PATH variables remain.
    echo "export PATH=\"$BIN_DIR:\$PATH\"" >>"$PROFILE"
  fi
}

function verify_search_path() {
  local cmd="$1"
  local expected="$2"
  local cwd="$3"

  # use a subshell to avoid clobbering the calling shell's $PATH
  local current=$(dirname $(env -i HOME=$HOME TERM=$TERM DIRENV_LOG_FORMAT= $SHELL -i -c "cd \"$cwd\"; which \"$cmd\""))

  if [[ "$current" != "$expected" ]]; then
    echo -e "\033[0;31mWARNING: $cmd in the $current directory has higher priority over $expected in the search path \$PATH.
    You can fix it by removing $cmd from $current or changing the order of directories in \$PATH.\033[0m"
  fi
}

# ==========//=========//=========//=========//==========//==========//==========
# versions
[[ -f "$ROOT/.versions" ]] && source "$ROOT/.versions"

function detect_rust_toolchain_version() {
    if [[ -f "rust-toolchain" ]]; then
        head -n 1 rust-toolchain | tr -d '"'
    elif [[ -f "rust-toolchain.toml" ]]; then
        sed -nE 's/^channel\s*=\s*"([^"]+)".*/\1/p' rust-toolchain.toml
    fi
}

RUST_TOOLCHAIN_VERSION=${RUST_TOOLCHAIN_VERSION:-1.88.0}
RUST_TOOLCHAIN_VERSION=${RUST_TOOLCHAIN_VERSION:-$(detect_rust_toolchain_version)}

CARGO_HOME=${CARGO_HOME:-"$HOME/.cargo"}
CARGO_BIN_DIR=${CARGO_BIN_DIR:-"$CARGO_HOME/bin"}

# https://docs.brew.sh/Installation
case $(uname -m) in
x86_64)
  BREW_BIN_DIR=${BREW_BIN_DIR:-"/usr/local/bin/"}
  ;;
arm64)
  BREW_BIN_DIR=${BREW_BIN_DIR:-"/opt/homebrew/bin"}
  ;;
esac

# ==========//=========//=========//=========//==========//==========//==========
# brew
function load_brew() {
  PATH="${BREW_BIN_DIR}:$PATH"
}

function ensure_brew() {
  load_brew
  if ! check_cmd brew; then
    box_out "MacOS: brew is not installed. please access https://brew.sh for installation."
    exit 1
  fi
}

# rosetta
function ensure_rosetta() {
  if ! /usr/bin/pgrep oahd >/dev/null 2>&1; then
    /usr/sbin/softwareupdate --install-rosetta --agree-to-license
  fi
}

# rustup
function load_rustup() {
  PATH="${CARGO_BIN_DIR}:$PATH"
}

function ensure_rustup() {
  load_rustup

  if ! check_cmd rustup; then
    # https://www.rust-lang.org/tools/install
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sed 's#/proc/self/exe#\/bin\/sh#g' | sh -s -- -y --no-update-default-toolchain
  fi
}

# cargo
function load_cargo() {
  PATH="${CARGO_BIN_DIR}:$PATH"
}

function ensure_cargo() {
  ensure_rustup

  rustup default ${RUST_TOOLCHAIN_VERSION}
  rustup component add clippy rustfmt
}

# crates
  function ensure_crates() {
  for crate in ${CARGO_CRATES}; do
    if ! command -v $crate &> /dev/null; then
      cargo install $crate;
    fi
  done
  }

# gh
function ensure_gh() {
  check_cmd gh && return

  case $OS in
  Linux)
    local cmd=$(
      cat <<-EOF
    apt install -y --no-install-recommends ca-certificates curl &&
      curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg &&
      chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg &&
      echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list >/dev/null &&
      apt-get update && apt install gh -y --no-install-recommends
EOF
    )
    try_sudo "$cmd"
    ;;
  Darwin)
    ensure_brew
    brew install gh
    ;;
  *)
    ;;
  esac
}

function ensure_profile() {
  local PROFILE=$(get_profile_path)

  # brew should be put at the first, otherwise the software installed with brew will has higher priority in search path.
  case $(uname -s) in
  Darwin)
    if [ ! -f "$PROFILE" ] || ! grep -q 'brew shellenv' "$PROFILE"; then
      cat <<-EOF | sed 's/^[[:space:]]*//' >>"$PROFILE"
      eval "\$(${BREW_BIN_DIR}/brew shellenv)"
EOF
    fi
    ;;
  esac

  local shell=$(get_shell_name)

  case $shell in
  sh | bash | zsh)
    if [ ! -f "$PROFILE" ] || ! grep -q 'direnv hook' "$PROFILE"; then
      cat <<-EOF | sed 's/^[[:space:]]*//' >>"$PROFILE"
      eval "\$(direnv hook ${shell})"
EOF
    fi
    ;;
  *)
    echo ": could not install direnv for ${shell}"
    exit 1
    ;;
  esac

  cat <<-EOF | sed 's/^[[:space:]]*//'

direnv has been setup in $PROFILE.

EOF
}

function get_envrc_path() {
  local PROFILE="$ROOT/.envrc"
  touch "$PROFILE"
  echo $PROFILE
}

function ensure_envrc() {
  local PROFILE=$(get_envrc_path)

  # Some softwares will be installed with cargo, so cargo bin dir should be output here ahead of others.
  # As a result, cargo bin dir will have a lower priotiry in search path.
  add_path_to_profile "$PROFILE" "${CARGO_BIN_DIR}"

  case "$(uname -s)-$(uname -m)" in
  Darwin-*)
    if [ ! -f "$PROFILE" ] || ! grep -q 'SDKROOT=' "$PROFILE"; then
      cat <<-'EOF' | sed 's/^[[:space:]]*//' >>"$PROFILE"
          export SDKROOT="$(xcrun --show-sdk-path --sdk macosx)"
EOF
    fi
    ;;
  esac

  local shell=$(get_shell_name)
  local message=$(
    cat <<-EOF | sed 's/^[[:space:]]*//'

start a new terminal session to use installed softwares, or run the command in the current terminal session:

  \033[0;34meval "\$(direnv hook $shell)"\033[0m

If you have trouble when building the workspace, please try to changing the order of directories in \$PATH.

EOF
  )
  echo -e "$message"
}

function verify_installs() {
  direnv allow "$ROOT"
  verify_search_path cargo "${CARGO_BIN_DIR}" "$ROOT"
}

function ensure_tools() {
  ensure_cargo
  ensure_crates
  ensure_gh
}

function install_on_macos() {
  ensure_rosetta
  ensure_brew

  brew install --quiet direnv

  ensure_tools
}

function install_on_linux() {
  local cmd=$(
    cat <<-EOF
    apt-get update &&
    apt-get install -y --no-install-recommends ca-certificates bash sudo curl wget git build-essential pkg-config direnv \
      g++ linux-libc-dev libclang-dev unzip libjemalloc-dev make time jq
EOF
  )
  try_sudo "$cmd"

  ensure_tools
}

case $OS in
Linux)
  install_on_linux
  ;;
Darwin)
  install_on_macos
  ;;
*)
  echo "Unknown"
  ;;
esac

ensure_profile
ensure_envrc
verify_installs
