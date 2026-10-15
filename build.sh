#!/bin/bash
# Build the explorer for the web (WebGPU). Run from anywhere; steps run in order.
#   ./build.sh                 every step: emsdk engine native wasm export
#   ./build.sh engine wasm     only those steps
#   ./build.sh test <project>  export tests/<project> to tests/<project>/build
# Builds run in the background with 2 jobs so the machine stays usable (JOBS=N to change).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")" && pwd)
JOBS=${JOBS:-2}
export EMCC_CORES=$JOBS
export BINARYEN_CORES=$JOBS
# macOS background QoS keeps the build on the efficiency cores; elsewhere just lower the priority.
if command -v taskpolicy >/dev/null; then LOWPRI="taskpolicy -c background nice -n 19"; else LOWPRI="nice -n 19"; fi
EMSDK_VERSION=4.0.11
NIGHTLY=nightly-2026-09-15
cd "$ROOT"

emsdk_python() {
  if [ -z "${EMSDK_PYTHON:-}" ]; then
    for py in python3.14 python3.13 python3.12 python3.11 python3.10 python3; do
      if command -v "$py" >/dev/null && "$py" -c 'import sys; sys.exit(sys.version_info < (3, 10))'; then
        export EMSDK_PYTHON=$(command -v "$py")
        break
      fi
    done
  fi
  [ -n "${EMSDK_PYTHON:-}" ] || { echo "Python 3.10+ is required for Emscripten"; exit 1; }
}

log() { echo -e "\n==> $*"; }

step_emsdk() {
  emsdk_python
  log "emsdk $EMSDK_VERSION"
  [ -f emsdk/emsdk ] || git submodule update --init emsdk
  (cd emsdk && ./emsdk install $EMSDK_VERSION && ./emsdk activate $EMSDK_VERSION)
  # Rust side modules export mangled std symbols that emscripten rejects; patch once.
  if ! grep -q "if not settings.SIDE_MODULE:" emsdk/upstream/emscripten/tools/emscripten.py; then
    (cd emsdk/upstream/emscripten && patch -p1 < "$ROOT/godot-explorer/web/emsdk-side-module-exports.patch")
  fi
}

step_engine() {
  emsdk_python
  log "engine web template (godotengine/bin)"
  (cd godotengine && source ../emsdk/emsdk_env.sh >/dev/null &&
    $LOWPRI scons platform=web target=template_debug dlink_enabled=yes webgpu=yes webgpu_precompile_shaders=no opengl3=no threads=no -j"$JOBS")
}

step_native() {
  log "explorer editor + native library"
  (cd godot-explorer && CARGO_BUILD_JOBS=$JOBS $LOWPRI cargo run -- install --no-templates &&
    CARGO_BUILD_JOBS=$JOBS $LOWPRI cargo run -- build --no-default-features)
}

step_wasm() {
  emsdk_python
  log "dclgodot.wasm"
  rustup toolchain list | grep -q "$NIGHTLY" || rustup toolchain install "$NIGHTLY" --component rust-src
  (cd godot-explorer/lib && source ../web/wasm-env.sh &&
    CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_STRIP=true CARGO_PROFILE_RELEASE_LTO=false \
    $LOWPRI cargo +"$NIGHTLY" build -j "$JOBS" --release -Zbuild-std=std,panic_abort \
      --target wasm32-unknown-emscripten --no-default-features --features use_pulse)
}

step_export() {
  log "export to godot-explorer/build/web"
  godot-explorer/web/tools/export-web.sh release ${EXPLORER_ARGS:---skip-lobby --guest-profile --realm=https://realm-provider-ea.decentraland.org/main --location=0,0}
}

step_preview() {
  log "export the editor preview (launch args supplied by the editor)"
  godot-explorer/web/tools/export-web.sh release
}

step_test() {
  local project=${1:?usage: ./build.sh test <project>}
  local dir="$ROOT/tests/$project"
  log "export tests/$project"
  if [ -f "$dir/gdext_hello.gdextension" ]; then
    (cd tests/gdext-hello && source "$ROOT/godot-explorer/web/wasm-env.sh" &&
      $LOWPRI cargo +"$NIGHTLY" build -j "$JOBS" --release --target wasm32-unknown-emscripten)
    mkdir -p "$dir/bin" && cp tests/gdext-hello/target/wasm32-unknown-emscripten/release/gdext_hello.wasm "$dir/bin/"
  fi
  mkdir -p "$dir/build"
  local log_file="$dir/build/export.log"
  if ! (cd "$dir" && $LOWPRI "$ROOT/godot-explorer/.bin/godot/godot4_bin" --headless --path . --export-debug web build/index.html) > "$log_file" 2>&1; then
    tail -n 60 "$log_file"
    exit 1
  fi
  if grep -Eq 'SCRIPT ERROR:|ERROR: (Cannot open file|Failed loading resource|Project export)' "$log_file"; then
    tail -n 60 "$log_file"
    exit 1
  fi
  # The stock editor doesn't emit renderingDriver; the WebGPU shell needs it.
  sed -i.bak 's/const GODOT_CONFIG = {/const GODOT_CONFIG = {"renderingDriver":"webgpu",/' "$dir/build/index.html" && rm "$dir/build/index.html.bak"
  echo "serve with: node godot-explorer/web/tools/serve-web.mjs tests/$project/build 8060"
}

if [ "${1:-}" = test ]; then
  step_test "${2:-}"
  exit
fi
for step in ${@:-emsdk engine native wasm export}; do
  "step_$step"
done
log "done. Serve with: node godot-explorer/web/tools/serve-web.mjs godot-explorer/build/web 8060"
