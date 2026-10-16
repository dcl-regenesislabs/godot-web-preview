# CLAUDE.md

Guidance for working in this repo: the Decentraland Godot explorer running in the browser on WebGPU.

## Context

`README.md` explains the project and how the build works, for people. This file is the operational part.

- **Engine:** Godot 4.6.2 fork plus a WebGPU RenderingDevice driver from
  [dwalter/godotwebgpu](https://github.com/dwalter/godotwebgpu), with our fixes. Forward Mobile renderer
  only, single-threaded.
- **Explorer:** the Rust library `dclgodot` built for `wasm32-unknown-emscripten`, loaded as a GDExtension
  side module.
- **Scenes:** scene JS runs in one Web Worker per scene, in place of deno/V8.
- **Findings, gaps and risks:** `godot-explorer/web/REPORT.md`.

**Settings not to "fix" without a reason:**
- **`threads=no`:** threads need COOP/COEP cross-origin isolation, and the driver assumes main-thread
  WebGPU calls.
- **`template_debug`:** it's the only template built so far.
- **Nightly Rust with `-Zbuild-std=std,panic_abort`:** the template has no C++ exception support, so std
  must be built with `panic=abort`.
- **`--no-default-features --features use_pulse` for wasm:** excludes deno and native LiveKit.
  Pulse selects WebTransport on web and ENet on native; scene messages use browser LiveKit.

## Layout

| Path | What |
|---|---|
| `godot-explorer/` | Git subtree from `decentraland/godot-explorer`, with browser changes |
| `godotengine/` | Git subtree from `decentraland/godotengine` on `4.6.2`, with WebGPU changes |
| `emsdk/` | Submodule → `emscripten-core/emsdk`. `build.sh` installs Emscripten 4.0.11 into it and patches it |
| `tests/` | `webgpu-smoke*` and `webgpu-mat`: small 3D scenes that print the driver and fps (`SMOKE ...` lines). `gdext-hello`: a minimal Rust GDExtension built for wasm, used by `webgpu-smoke-min` |
| `build.sh` | Every engine/runtime build step |
| `packages/runtime/` | Versioned npm artifacts, integrity checks and OIDC release setup |

The scripts expect these three folders side by side; they find each other by relative path.

## Build

```bash
./build.sh                  # every step: emsdk engine native wasm export
./build.sh wasm export      # only some steps (after changing Rust code)
./build.sh wasm preview     # editor preview; launch args come from the editor
./build.sh test webgpu-smoke
```

| Step | Does | Output |
|---|---|---|
| `emsdk` | Installs Emscripten 4.0.11 and applies `godot-explorer/web/emsdk-side-module-exports.patch` | `emsdk/upstream/` |
| `engine` | `scons platform=web target=template_debug dlink_enabled=yes webgpu=yes webgpu_precompile_shaders=no opengl3=no threads=no` | `godotengine/bin/godot.web.template_debug.wasm32.nothreads.dlink.zip` |
| `native` | `cargo run -- install` (Godot editor, protoc, Sentry addon) and a native build of the library, which the editor needs to import the project | `godot-explorer/.bin/`, `godot-explorer/lib/target/` |
| `wasm` | `dclgodot` for `wasm32-unknown-emscripten` with nightly Rust and `-Zbuild-std=std,panic_abort` | `godot-explorer/lib/target/wasm32-unknown-emscripten/release/dclgodot.wasm` |
| `preview` | Exports the editor runtime without baked launch arguments | `godot-explorer/build/web/` |
| `export` | Exports the `web` preset and injects `"renderingDriver":"webgpu"` plus the explorer args into `index.html` | `godot-explorer/build/web/` |

- **Rebuilds:** after changing engine C++, run `./build.sh engine export`. After changing explorer Rust or
  `scene_worker.js`, run `./build.sh wasm export`. After GDScript or asset changes, `./build.sh export` (or `./build.sh preview` for the editor) is enough.
  Export imports assets first and rejects script/resource errors even if Godot exits zero.
- **CPU:** local builds run with 2 jobs, under `nice -n 19` and, on macOS, `taskpolicy -c background`
  (efficiency cores only), so the machine stays usable. `JOBS=N` changes the job count. Run any manual
  local build the same way: never at full parallelism. Dedicated CI runners set
  `BUILD_PRIORITY=normal` and use their three available macOS cores.
- **Explorer args:** `EXPLORER_ARGS="--skip-lobby --guest-profile --realm=https://realm-provider-ea.decentraland.org/main --location=52,-52" ./build.sh export`
  changes the args baked into `index.html`. Without `--realm=` the explorer opens its default test realm,
  not Genesis City. `--guest-profile` is needed: it creates a disposable wallet,
  and without one, signed fetch fails.

### Toolchain requirements

- **Python 3.10+:** required only for Emscripten steps. `build.sh` selects Python 3.10+
  or uses `EMSDK_PYTHON`; export-only steps need no Emscripten installation.
- **Rust:** the wasm build uses `nightly-2026-09-15` with `rust-src`, which `build.sh` installs. Native
  builds use the pinned 1.90.
- **gdext:** its prebuilt bindings are 64-bit only, so `godot-explorer/web/wasm-env.sh` points
  `GODOT4_GDEXTENSION_JSON` at the 4.5 API JSON from the cargo registry. Run a native build first so it's
  downloaded.
- **Shaders:** the preview uses runtime WGSL conversion. `webgpu_precompile_shaders=no` skips the
  optional native Tint/glslang build and emits an empty lookup table. This preserves the tested
  runtime path and avoids unnecessary host-tool compilation.
- **Don't add `-Zlink-native-libraries=no`** to the explorer build. It drops the `EM_JS` C archives
  (fetch, websocket, scene bridge).

## Run and test

```bash
# In your own Chrome
node godot-explorer/web/tools/serve-web.mjs godot-explorer/build/web 8060   # open http://localhost:8060/

# Headless Chrome: console log on stdout, screenshot at the end (needs `npm install` in godot-explorer/web/tools once)
node godot-explorer/web/tools/run-web.mjs godot-explorer/build/web 150 shot.png --size=1728x1000 --dpr=2 \
  "--actions=60:shot:a.png;90:click:640,360;100:key:Enter;110:eval:devicePixelRatio"
```

- **Runner output:** `run-web.mjs` prints every console line with a timestamp, failed requests
  (`[reqfail]`), HTTP errors with POST bodies (`[http 4xx]`) and page errors.
- **What a healthy explorer run shows:**
  - `WebGPU 1.0 - Forward Mobile`, and no `WebGPU uncaptured error` lines;
  - scene logs as `[Rust:dclgodot::web::scene_host] [scene N] ...`;
  - `[PERF] fps=...` lines;
  - a screenshot of Genesis Plaza with the HUD.
- **Use `localhost`:** some Decentraland APIs allow `localhost` but not `127.0.0.1` in CORS.
- **Smoke tests:** `./build.sh test webgpu-smoke`, then serve or run `tests/webgpu-smoke/build`. They pass
  when they print `SMOKE driver=webgpu` and `SMOKE fps=`, with no `WebGPU uncaptured error` lines.
- **URL options:** `?maxpixels=N` caps the canvas at N device pixels (default 1920×1080).
  `?maxpixels=native` renders at full screen resolution.

## Where the web code lives

- **Engine (`godotengine/`)**
  - `drivers/webgpu/`: the RenderingDevice driver (SPIR-V → WGSL through vendored Tint in
    `thirdparty/tint`).
  - `platform/web/js/engine/engine.js`: requests the WebGPU device and its limits.
  - `platform/web/display_server_web.cpp`: canvas and surface sizing.
  - When a shader fails to convert, the log names it and prints `WEBGPU_FAILED_SPIRV_B64:<base64>`. Replay
    it offline with `drivers/webgpu/tint_cli/build.sh` (`bin/tint_convert_cli`).
- **Explorer (`godot-explorer/`)**
  - `lib/src/web/`: the web runtime. Highlights:
    - `scene_host.rs` + `scene_worker.js`: the per-scene Worker, which provides the `Deno.core.ops` surface
      so `lib/src/dcl/js/js_modules` run unchanged;
    - `fs`: a `tokio::fs` stand-in.
  - `lib/web-reqwest`: the reqwest subset over `fetch()`.
  - `lib/web-websocket`: a browser WebSocket.
  - `lib/web-scene`: the Worker bridge (`EM_JS` in `scene.c`). Each `EM_JS` function also needs an
    `--export=__em_js__<name>` line in `lib/.cargo/config.toml`.
  - `lib/src/dcl/scene_ops.rs`: op bodies shared by the deno ops and the web host (player data, signed
    fetch).
  - `lib/vendor/rpc-rust`: dcl-rpc with tokio networking made optional so it builds for wasm.
  - Everything web-only sits behind `cfg(target_family = "wasm")`. Native builds must keep compiling:
    `cd godot-explorer/lib && cargo check` and `cargo check --no-default-features`.

### Adding a scene API on web

1. In `scene_worker.js`, add the op to `opsImpl` as `call('<name>', [args])`.
2. Answer it in `on_call` in `scene_host.rs`.
3. If deno has its own copy of the op body, move it to `dcl/scene_ops.rs` so both runtimes share it.

`decentraland/bevy-explorer` has the complete design to copy for the remaining ops: its op bodies are
written against a trait and run in a second wasm inside each Worker
(`crates/dcl_scene_wasm`, `deploy/web/engine/sandbox_worker.js`).

## Committing

Changes inside `godot-explorer/` or `godotengine/` are ordinary commits in this
repository. Use a feature branch and keep upstream imports separate from browser
changes. Run `node scripts/upstream.mjs sync` for upstream updates; see
`docs/UPSTREAMS.md` for pairing and conflict handling. Never commit `build/`, `bin/`
or `target/` output. User instructions about committing or pushing are authoritative.

## Known gaps

- **Scene APIs:** gameplay movement, emotes, confirmation dialogs, events, world time
  and locale are supported. Portable experiences, file reads and wallet utilities
  remain excluded. Local preview scene messages use browser LiveKit; Pulse carries
  player state through WebTransport.
- **Compiled out:** native voice/LiveKit, video, and Sentry. Pulse uses browser WebTransport.
- **Content cache:** it lives in memory (Emscripten MEMFS).
- **CORS:** `marketing-files.decentraland.org` only allows `https://decentraland.org`, so in-world banners
  fail elsewhere.
- **Engine:** a scene UBO size mismatch shows up only in the sky + glow smoke test.
- **Coverage:** only tested in Chrome on macOS (Apple M4 Pro).
