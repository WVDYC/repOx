#!/usr/bin/env bash
# Generate shell completion scripts for repOx
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="${REPO_ROOT}/target/release/repox"
OUTPUT_DIR="${REPO_ROOT}/completions"

if [ ! -f "${BIN_PATH}" ]; then
    echo "Building release binary..."
    cargo build --release --manifest-path "${REPO_ROOT}/Cargo.toml"
fi

mkdir -p "${OUTPUT_DIR}"

echo "Generating shell completions into ${OUTPUT_DIR}/..."
"${BIN_PATH}" --completions bash > "${OUTPUT_DIR}/repox.bash"
"${BIN_PATH}" --completions zsh > "${OUTPUT_DIR}/_repox"
"${BIN_PATH}" --completions fish > "${OUTPUT_DIR}/repox.fish"
"${BIN_PATH}" --completions elvish > "${OUTPUT_DIR}/repox.elv"
"${BIN_PATH}" --completions powershell > "${OUTPUT_DIR}/_repox.ps1"

echo "✓ Generated completions for Bash, Zsh, Fish, Elvish, and PowerShell!"
