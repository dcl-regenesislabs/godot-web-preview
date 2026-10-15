#!/bin/bash
# Export the explorer to web (WebGPU) with the wasm dclgodot from lib/target.
# usage: web/tools/export-web.sh [release|debug] [godot args...]
# e.g.   web/tools/export-web.sh release --skip-lobby --guest-profile --location=0,0
set -euo pipefail
EXPLORER=$(cd "$(dirname "$0")/../.." && pwd)
PROFILE=${1:-release}
shift || true
ARGS_JSON=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1:]))' "$@")

mkdir -p "$EXPLORER/lib/target/libdclgodot_web" "$EXPLORER/build/web"
npm ci --prefix "$EXPLORER/web/tools" --ignore-scripts --no-audit
cp "$EXPLORER/web/tools/node_modules/livekit-client/dist/livekit-client.umd.js" "$EXPLORER/build/web/"
cp "$EXPLORER/web/tools/node_modules/livekit-client/LICENSE" "$EXPLORER/build/web/livekit-LICENSE.txt"
cp "$EXPLORER/web/scene-comms.js" "$EXPLORER/build/web/"
cp "$EXPLORER/web/pulse.js" "$EXPLORER/build/web/"
cp "${DCLGODOT_WASM:-$EXPLORER/lib/target/wasm32-unknown-emscripten/$PROFILE/dclgodot.wasm}" "$EXPLORER/lib/target/libdclgodot_web/dclgodot.wasm"

cd "$EXPLORER/godot"
LOG="$EXPLORER/build/export-web.log"
if ! nice -n 19 "${GODOT_BIN:-../.bin/godot/godot4_bin}" --headless --editor --path . --import > "$EXPLORER/build/import-web.log" 2>&1; then
  tail -n 60 "$EXPLORER/build/import-web.log"
  exit 1
fi
if ! nice -n 19 "${GODOT_BIN:-../.bin/godot/godot4_bin}" --headless --path . --export-debug web ../build/web/index.html > "$LOG" 2>&1; then
  tail -n 60 "$LOG"
  exit 1
fi
if grep -Eq 'SCRIPT ERROR:|ERROR: (Cannot open file|Failed loading resource|Project export)|panicked at' "$LOG"; then
  grep -E 'SCRIPT ERROR:|ERROR:|panicked at' "$LOG" | tail -n 30
  exit 1
fi

# The stock editor doesn't emit renderingDriver; the WebGPU shell needs it to request a device.
python3 - "$EXPLORER/build/web/index.html" "$ARGS_JSON" <<'PY'
import sys, re, json
path, args = sys.argv[1], sys.argv[2]
s = open(path).read()
match = re.search(r'const (GODOT_CONFIG|config) = (\{[^\n]+\});', s)
if not match:
    raise SystemExit('Export shell is missing the Godot configuration')
config = json.loads(match[2])
config.update(renderingDriver='webgpu', args=json.loads(args))
s = s[:match.start()] + 'const ' + match[1] + ' = ' + json.dumps(config) + ';' + s[match.end():]
open(path, 'w').write(s)
PY
ls -la "$EXPLORER/build/web" | grep -E "wasm|pck|html"
