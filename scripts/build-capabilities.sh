#!/usr/bin/env bash
# Build every Ocinye capability to wasm32-wasip1.
#
# Capabilities live outside the host Cargo workspace on purpose: they target
# WebAssembly, and including them would force a wasm build into every native
# `cargo build` (ADR-0005).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if ! rustup target list --installed | grep -qx 'wasm32-wasip1'; then
    echo "Installing the wasm32-wasip1 target..."
    rustup target add wasm32-wasip1
fi

# Built into the repository's shared target directory, not each crate's own.
# Without --target-dir, `--manifest-path` builds into the crate's directory, and
# the Capability Runtime tests would look for the component somewhere else.
for manifest in wasm/capabilities/*/Cargo.toml; do
    capability="$(basename "$(dirname "$manifest")")"
    echo "Building capability: $capability"
    cargo build --release --target wasm32-wasip1 \
        --manifest-path "$manifest" \
        --target-dir "$root/target"
done

echo
echo "Built components:"
find target/wasm32-wasip1/release -maxdepth 1 -name '*.wasm' -exec ls -lh {} \;
