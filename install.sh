#!/usr/bin/env bash
set -euo pipefail

# NALA-halogen-rs Zero-Python Native Rust Installer (macOS / Linux)
ROOT="$(cd "$(dirname "$0")" && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust toolchain (cargo) not found. Install from https://rustup.rs" >&2
  exit 1
fi

echo "Building nala-halogen-rs (100% Pure Safe Rust, Edition 2024, 0 Crates)..."
cargo build --release --manifest-path "$ROOT/Cargo.toml"

DEST="${NALA_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$DEST"
cp "$ROOT/target/release/nala-halogen-rs" "$DEST/nala-halogen-rs"
chmod +x "$DEST/nala-halogen-rs"

"$DEST/nala-halogen-rs" --version
echo "Installed to $DEST/nala-halogen-rs"
