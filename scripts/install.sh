#!/bin/sh
set -eu

REPO="Patrick0308/statichub"
SCOPE="cli"
VERSION="latest"
BIN_DIR="${HOME}/.local/bin"

usage() {
  cat <<'USAGE'
StaticHub installer (Unix/Linux/macOS)

Usage:
  curl -sSL https://raw.githubusercontent.com/Patrick0308/statichub/main/scripts/install.sh | sh
  curl -sSL https://raw.githubusercontent.com/Patrick0308/statichub/main/scripts/install.sh | sh -s server
  curl -sSL https://raw.githubusercontent.com/Patrick0308/statichub/main/scripts/install.sh | sh -s both
  curl -sSL https://raw.githubusercontent.com/Patrick0308/statichub/main/scripts/install.sh | sh -s -- -s both

Options:
  -s, --scope <cli|server|both>   Install target (default: cli)
      --version <tag>             Release version tag (default: latest)
      --bin-dir <path>            Install directory (default: ~/.local/bin)
  -h, --help                      Show this help
USAGE
}

is_scope() {
  [ "$1" = "cli" ] || [ "$1" = "server" ] || [ "$1" = "both" ]
}

if [ "$#" -gt 0 ] && is_scope "$1"; then
  SCOPE="$1"
  shift
fi

while [ "$#" -gt 0 ]; do
  case "$1" in
    -s|--scope)
      [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; exit 1; }
      SCOPE="$2"
      shift 2
      ;;
    --version)
      [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; exit 1; }
      VERSION="$2"
      shift 2
      ;;
    --bin-dir)
      [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; exit 1; }
      BIN_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

is_scope "$SCOPE" || { echo "Invalid scope: $SCOPE (expected cli|server|both)" >&2; exit 1; }

OS="$(uname -s)"
ARCH="$(uname -m)"
TARGET=""

case "$OS" in
  Darwin)
    case "$ARCH" in
      x86_64) TARGET="x86_64-apple-darwin" ;;
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      *) echo "Unsupported macOS architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64|amd64) TARGET="x86_64-linux-musl" ;;
      *) echo "Unsupported Linux architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

URL="https://github.com/${REPO}/releases/download/${VERSION}/statichub-${TARGET}.tar.gz"
if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/${REPO}/releases/latest/download/statichub-${TARGET}.tar.gz"
fi

if command -v curl >/dev/null 2>&1; then
  DOWNLOADER='curl -fsSL'
elif command -v wget >/dev/null 2>&1; then
  DOWNLOADER='wget -qO-'
else
  echo "Need curl or wget to download binaries." >&2
  exit 1
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

mkdir -p "$BIN_DIR"

echo "Downloading: $URL"
# shellcheck disable=SC2086
if ! $DOWNLOADER "$URL" | tar -xz -C "$TMP_DIR"; then
  echo "Download or extraction failed. Check release assets and platform support:" >&2
  echo "https://github.com/${REPO}/releases" >&2
  exit 1
fi

install_bin() {
  src="$1"
  name="$2"
  if [ ! -f "$src" ]; then
    echo "Expected binary not found in archive: $name" >&2
    exit 1
  fi
  chmod +x "$src"
  cp "$src" "$BIN_DIR/$name"
  echo "Installed $name -> $BIN_DIR/$name"
}

case "$SCOPE" in
  cli)
    install_bin "$TMP_DIR/statichub" "statichub"
    ;;
  server)
    install_bin "$TMP_DIR/statichub-server" "statichub-server"
    ;;
  both)
    install_bin "$TMP_DIR/statichub" "statichub"
    install_bin "$TMP_DIR/statichub-server" "statichub-server"
    ;;
esac

case ":$PATH:" in
  *":$BIN_DIR:"*)
    ;;
  *)
    echo ""
    echo "Note: $BIN_DIR is not in your PATH. Add this line to your shell profile:"
    echo "  export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac

echo ""
echo "Done. Verify with:"
case "$SCOPE" in
  cli) echo "  statichub version" ;;
  server) echo "  statichub-server version" ;;
  both)
    echo "  statichub version"
    echo "  statichub-server version"
    ;;
esac
