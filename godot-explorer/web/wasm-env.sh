# Source before a wasm build of dclgodot: `source web/wasm-env.sh` from the repo root.
# Expects emsdk 4.0.11 (with emsdk-side-module-exports.patch applied) next to this repo,
# or at $EMSDK_DIR.
_WEB_DIR=$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)
EMSDK_DIR=${EMSDK_DIR:-$(cd "$_WEB_DIR/../.." && pwd)/emsdk}
# emsdk needs python 3.10+; macOS ships 3.9 as python3.
if [ -z "${EMSDK_PYTHON:-}" ]; then
  for _py in python3.13 python3.12 python3.11 python3.10; do
    command -v $_py >/dev/null && export EMSDK_PYTHON=$(command -v $_py) && break
  done
  unset _py
fi
source "$EMSDK_DIR/emsdk_env.sh" >/dev/null 2>&1
export CC_wasm32_unknown_emscripten=emcc CXX_wasm32_unknown_emscripten=em++ AR_wasm32_unknown_emscripten=emar
# gdext's prebuilt bindings are 64-bit only; generate them from the 4.5 API JSON.
export GODOT4_GDEXTENSION_JSON=${GODOT4_GDEXTENSION_JSON:-$(ls -d "$HOME"/.cargo/registry/src/*/gdextension-api-0.3.0/versions/4.5/res/extension_api.json 2>/dev/null | head -1)}
[ -z "${LIBCLANG_PATH:-}" ] && [ -d /Library/Developer/CommandLineTools/usr/lib ] && export LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib
export BINDGEN_EXTRA_CLANG_ARGS_wasm32_unknown_emscripten="--target=wasm32-unknown-emscripten --sysroot=$EMSDK/upstream/emscripten/cache/sysroot"
export PATH="$_WEB_DIR/../.bin/protoc/bin:$PATH"
unset _WEB_DIR
