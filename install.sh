#!/bin/sh
# repOx Universal Installer
# Compatible with POSIX sh, bash, zsh, dash
# Usage: curl -fsSL https://raw.githubusercontent.com/WVDYC/repOx/main/install.sh | sh

set -eu

REPO="WVDYC/repOx"
BINARY_NAME="repox"

# Colors for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

info() {
    printf "${CYAN}${BOLD}==>${NC} ${BOLD}%s${NC}\n" "$1"
}

success() {
    printf "${GREEN}${BOLD}✓${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}${BOLD}!${NC} %s\n" "$1"
}

error() {
    printf "${RED}${BOLD}error:${NC} %s\n" "$1" >&2
    exit 1
}

# 1. Detect OS
OS_RAW="$(uname -s)"
case "${OS_RAW}" in
    Linux*)     OS="unknown-linux-gnu" ;;
    Darwin*)    OS="apple-darwin" ;;
    *)          error "Unsupported operating system: ${OS_RAW}. repOx supports macOS and Linux." ;;
esac

# 2. Detect Architecture
ARCH_RAW="$(uname -m)"
case "${ARCH_RAW}" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)  ARCH="aarch64" ;;
    *)              error "Unsupported architecture: ${ARCH_RAW}. repOx supports x86_64 and aarch64." ;;
esac

TARGET="${ARCH}-${OS}"

# 3. Determine Version Tag
if [ -n "${REPOX_VERSION:-}" ]; then
    TAG="${REPOX_VERSION}"
    info "Targeting specified version: ${TAG}"
else
    info "Querying latest release from GitHub API (${REPO})..."
    LATEST_JSON="$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)"
    
    if [ -n "${LATEST_JSON}" ]; then
        TAG="$(printf '%s' "${LATEST_JSON}" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)"
    fi

    if [ -z "${TAG:-}" ]; then
        TAG="v0.1.0"
        warn "Could not resolve latest release via GitHub API. Defaulting to ${TAG}."
    else
        success "Latest release found: ${TAG}"
    fi
fi

# 4. Construct Download URLs
TARBALL="repox-${TAG}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${TARBALL}"
CHECKSUM_URL="https://github.com/${REPO}/releases/download/${TAG}/${TARBALL}.sha256"

# 5. Prepare Temporary Directory
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'repox-install')"
cleanup() {
    rm -rf "${TMP_DIR}"
}
trap cleanup EXIT INT TERM

info "Downloading ${TARBALL}..."
if ! curl -fSL --progress-bar -o "${TMP_DIR}/${TARBALL}" "${DOWNLOAD_URL}"; then
    error "Failed to download release archive from: ${DOWNLOAD_URL}"
fi

# 6. Verify Checksum
info "Verifying SHA-256 checksum..."
if curl -fsSL -o "${TMP_DIR}/${TARBALL}.sha256" "${CHECKSUM_URL}" 2>/dev/null; then
    EXPECTED_SHA="$(awk '{print $1}' "${TMP_DIR}/${TARBALL}.sha256" | tr -d ' \n\r')"
    
    if command -v sha256sum >/dev/null 2>&1; then
        ACTUAL_SHA="$(sha256sum "${TMP_DIR}/${TARBALL}" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
        ACTUAL_SHA="$(shasum -a 256 "${TMP_DIR}/${TARBALL}" | awk '{print $1}')"
    else
        warn "Neither 'sha256sum' nor 'shasum' found; skipping checksum verification."
        ACTUAL_SHA=""
    fi

    if [ -n "${ACTUAL_SHA}" ]; then
        if [ "${EXPECTED_SHA}" != "${ACTUAL_SHA}" ]; then
            error "Checksum verification failed! Expected: ${EXPECTED_SHA}, Actual: ${ACTUAL_SHA}"
        fi
        success "Checksum verified: ${ACTUAL_SHA}"
    fi
else
    warn "Checksum file not available for this release; continuing without verification."
fi

# 7. Unpack Binary
info "Unpacking release..."
tar -xzf "${TMP_DIR}/${TARBALL}" -C "${TMP_DIR}"

UNPACKED_BIN="$(find "${TMP_DIR}" -type f -name "${BINARY_NAME}" | head -n 1)"
if [ -z "${UNPACKED_BIN}" ] || [ ! -f "${UNPACKED_BIN}" ]; then
    error "Could not find '${BINARY_NAME}' binary in unpacked archive."
fi
chmod +x "${UNPACKED_BIN}"

# 8. Determine Installation Path
DEFAULT_INSTALL_DIR="/usr/local/bin"
USER_INSTALL_DIR="${HOME}/.local/bin"

if [ -w "${DEFAULT_INSTALL_DIR}" ]; then
    INSTALL_DIR="${DEFAULT_INSTALL_DIR}"
    USE_SUDO=0
elif command -v sudo >/dev/null 2>&1 && [ -t 0 ]; then
    INSTALL_DIR="${DEFAULT_INSTALL_DIR}"
    USE_SUDO=1
else
    INSTALL_DIR="${USER_INSTALL_DIR}"
    USE_SUDO=0
    mkdir -p "${INSTALL_DIR}"
fi

info "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."
if [ "${USE_SUDO}" -eq 1 ]; then
    sudo cp "${UNPACKED_BIN}" "${INSTALL_DIR}/${BINARY_NAME}"
    sudo chmod 755 "${INSTALL_DIR}/${BINARY_NAME}"
else
    cp "${UNPACKED_BIN}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod 755 "${INSTALL_DIR}/${BINARY_NAME}"
fi

# 9. Verify Installation
if command -v repox >/dev/null 2>&1; then
    INSTALLED_VER="$(repox --version 2>/dev/null || echo "repox ${TAG}")"
    success "${INSTALLED_VER} installed successfully!"
else
    success "repOx installed to ${INSTALL_DIR}/${BINARY_NAME}"
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            warn "${INSTALL_DIR} is not in your PATH."
            warn "Add it to your shell configuration:"
            printf "    export PATH=\"%s:\$PATH\"\n\n" "${INSTALL_DIR}"
            ;;
    esac
fi

printf "\n${GREEN}${BOLD}⚡ Quickstart:${NC}\n"
printf "  ${BOLD}repox -c${NC}             # Copy whole repository context to clipboard\n"
printf "  ${BOLD}repox -i -p claude${NC}   # Launch interactive TUI with live token gauge\n\n"
