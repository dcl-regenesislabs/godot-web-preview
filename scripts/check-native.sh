#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../godot-explorer/lib"
export PATH="$(cd ../.bin/protoc/bin && pwd):$PATH"
protoc --version
cargo test --locked --lib --no-default-features --features use_pulse comms::pulse:: -j "${JOBS:-2}"
cargo test --locked --lib --no-default-features --features use_pulse scene_ -j "${JOBS:-2}"
if [ "${CHECK_NATIVE_COMPATIBILITY:-false}" = true ]; then
  cargo check --locked --no-default-features --features use_deno -j "${JOBS:-2}"
fi
