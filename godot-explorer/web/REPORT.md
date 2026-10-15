# godot-explorer on the web (WebGPU): spike report

**Date:** 2026-10-12 · **Status:** finished.

## Verdict

**It's viable.** In one day, the real explorer ran in Chrome on WebGPU with:
- our Godot 4.6.2 fork, plus dwalter's WebGPU driver and four fixes;
- the full `dclgodot` Rust library compiled to wasm;
- scene JavaScript running in Web Workers instead of deno/V8.

It gets through boot and the lobby into the world, and it renders **Genesis Plaza from its real 2.4 MB SDK7 scene code**: models, lights, materials and the scene's React-ECS UI. That frame had about 840 draws and **zero WebGPU validation errors**, holding the client's 30 fps cap on an M4 Pro in headless Chrome 154.

What remains is mostly bounded engineering: bridging the APIs scenes use, comms and voice, auth, and a lot of hardening. The open risks are performance on weak devices (everything except scene JS runs on one thread), memory (wasm32 caps at 4 GB and the content cache currently lives in RAM), download size, and owning an AI-written WebGPU driver.

## About the dwalter/godotwebgpu fork

- **Same base as ours (4.6.2).** Its diff against upstream is self-contained: `drivers/webgpu/` (~9k-line RenderingDevice driver), vendored Tint + SPIRV-Tools, and modest patches to `servers/rendering` and `platform/web`. **It applied onto our fork with no conflicts.**
- **Only Forward Mobile, only single-threaded (`threads=no`).** Both match what the explorer uses (`rendering_method="mobile"`).
- **Its tip is newer than its own validation.** The last commits (2026-05-12) replaced its naga converter with Tint, and the "zero GPU errors" claims predate that. Against Chrome 154, a trivial 3D scene was **black** and every material was **white**. I fixed four bugs (below) in about 2 hours, with no further GPU errors through Genesis Plaza. One mismatch remains, seen only in a sky+glow test scene.
- **Recommendation: use it as the base, own it in our fork, and treat it as a fast start rather than a finished product.** The 7.7k-line driver is AI-generated, has one maintainer and 69 stars. Upstream Godot still has no WebGPU renderer.

## What was done

### Engine (`godotengine`, branch `spike/webgpu-web`)

| Commit | What |
|---|---|
| `63567b205c` | Import dwalter webgpu-4.6.2 @ f329e39 (no conflicts) |
| `e91eb42775` | **Depth textures sampled through a filtering sampler** (omni/spot PCSS blocker search) made every scene pipeline invalid → rewrite those WGSL reads to `textureLoad`. **Tint `AtomicVec2uToFromU64`** rebuilt runtime arrays without their stride → corrupted struct size → every material shader failed (white materials). Failing shaders are now named, and their SPIR-V is dumped for offline replay. |
| `0b4ce58a03` | BC7/ASTC missing from the sRGB view-format pairing (every `source_color` BC7 texture failed). Request the adapter's `maxInterStageShaderVariables` (28 on Apple; the WebGPU default of 16 broke three terrain shaders). |

Build: `scons platform=web target=template_debug dlink_enabled=yes webgpu=yes opengl3=no threads=no` (Emscripten 4.0.11).

### Rust library (`godot-explorer`, branch `spike/webgpu-web`)

Native builds are unchanged: everything is behind `cfg(target_family = "wasm")`, and native `cargo check` passes.

- **Toolchain:** nightly Rust (wasm build only) with `-Zbuild-std=std,panic_abort`, because the Godot template has no C++ exception support. gdext uses `experimental-wasm-nothreads` and `api-custom-json`, since its prebuilt bindings are 64-bit only. The rustflags (side module, panic=abort, `EM_JS` exports) are in `lib/.cargo/config.toml`.
- **Networking:**
  - `lib/web-reqwest`: the reqwest API subset we use, over browser `fetch()`. Call sites are unchanged.
  - `lib/web-websocket`: browser WebSocket as async Rust. The social service uses it through `dcl-rpc`, which is vendored with tokio networking made optional (`lib/vendor/rpc-rust`, via `[patch]`).
- **Runtime:** a single-threaded tokio runtime driven from Godot's `_process` every frame. Thread spawns are replaced on web (floating-islands mesh worker, `spawn_blocking`, the scene thread).
- **Filesystem:** `web::fs` provides the `tokio::fs` API over Emscripten's in-memory filesystem.
- **Scenes (`web::scene_host` + `scene_worker.js` + `lib/web-scene`):**
  - Each scene runs its `main.js` in a dedicated Web Worker.
  - The Worker implements the `Deno.core` surface, so the existing `js_modules` and the `main.js` loader run **unmodified**.
  - CRDT frames are bridged both ways with the same Rust code paths the deno ops use.
- **Small items:** SNTP is disabled (browsers have no UDP), Ethereum JSON-RPC goes over HTTPS, `LocalWallet` signing is synchronous, and deno-only metrics are gated.
- **Godot project:** a web export preset, a `web.*.wasm32` entry in `dclgodot.gdextension`, `rendering_method.web=mobile` / `driver.web=webgpu`, and one line in `global.gd` that adds the tokio runtime node to the tree on web.

### Sizes (current debug engine template)

| File | Raw | Brotli |
|---|---|---|
| Engine (`index.side.wasm`, **debug**) | 44.6 MB | 9.3 MB |
| `dclgodot.wasm` (release, no LTO) | 12.2 MB | 2.5 MB |
| `index.pck` (all project assets) | 111.8 MB | 52.0 MB |

## Scene JS: our Worker shim vs bevy-explorer

bevy-explorer (`decentraland/bevy-explorer`) already ships the complete web scene runtime. It is the model to copy when this goes past the spike.

| | This spike | bevy-explorer |
|---|---|---|
| Where scene JS runs | One Web Worker per scene | One Web Worker per scene (`deploy/web/engine/sandbox_worker.js`) |
| Where ops run | Worker JS forwards CRDT and supported API requests to the Godot Wasm host | A **second wasm module per Worker** (`crates/dcl_scene_wasm`) exposing every `op_*` through wasm-bindgen |
| Op code shared with native | CRDT, player queries, signed fetch, gameplay RPC handlers and scene events | **All of it.** The op bodies in `crates/dcl/src/js/*.rs` are written against a small `State` trait instead of deno's `OpState`. Deno implements it natively; `crates/dcl_wasm/src/inner/gotham_state.rs` implements it on wasm, and `op_wrappers/` exports them. |
| Engine ↔ scene channel | `postMessage` of raw CRDT frames | A typed relay (`relay.rs`, `relay_proto.rs`, msgpack) carrying renderer updates, global CRDT, RPC and system-API calls |
| Sandboxing | None | Allowlisted globals; `BroadcastChannel`, `Worker`, storage, IndexedDB, Cache and service worker removed; kill and shutdown tokens |

**To copy it:**
1. Refactor `lib/src/dcl/js/*.rs` so the op bodies take a trait instead of `deno_core::OpState`, as bevy did. Native deno keeps working unchanged.
2. Build those ops as a second wasm module for the Worker, then replace the `opsImpl` table in `scene_worker.js` with bevy's generic op wrapper.
3. Take bevy's sandbox scrub as is.

That brings events, signed fetch, players, restricted actions and comms along, instead of porting about 50 ops by hand.

**Caveat:** bevy's Rust side builds with wasm-bindgen for `wasm32-unknown-unknown`. Ours is an Emscripten side module loaded by Godot. The scene runtime is a separate wasm in its own Worker, so it can still use wasm-bindgen; the Godot side stays Emscripten.

## Direction: our own engine fork

The team plans to fully own the engine fork, developing it with AI and cherry-picking from upstream Godot as needed. That fits this result: the WebGPU driver, the Tint fixes and the web template patches already live only in our fork.

## Not done or not bridged yet (expected next work)

- **Scene APIs:** player data, signed fetch, local preview input, pointer events, scene messages and gameplay restricted actions are bridged. Movement/emotes/navigation use the existing renderer RPC handlers; native and browser events share `dcl/scene_events.rs`. Portable experiences, file reads, wallet operations, external URL/NFT dialogs and per-scene logs to the renderer remain missing. Movement interpolation is also absent from the native handler.
- **Comms and voice:** local preview scene messages use browser LiveKit; Pulse player state uses WebTransport with the native authentication/decoder. Native ENet and voice remain excluded from web builds.
- **Auth:**
  - thirdweb guest login answers 401 "Unauthorized domain" (a dashboard allowlist setting).
  - The sign-in flow is untested.
  - The spike enters the world with `--skip-lobby --guest-profile`: a disposable wallet and random profile, enough for signed fetch.
- **Other compiled-out or disabled pieces:** video (no-op backend), Sentry (JS bridge missing), Android/iOS plugins.
- **Content cache:** `MEMFS` keeps every downloaded asset in RAM. It needs IndexedDB/Cache API and should decode straight from memory.
- **Threads:** only `threads=no` was tested. A threaded build needs COOP/COEP (cross-origin isolation, which affects every third-party resource), and dwalter's driver assumes main-thread WebGPU calls.
- **Browsers and devices:** only headless Chrome 154 on an M4 Pro (macOS) was tested. Firefox, Safari, Windows, Android and iOS are untested.
- **CORS:** `marketing-files.decentraland.org` (in-world banners) only allows the `https://decentraland.org` origin, so the browser blocks it everywhere else. The web origin has to be added on the server. Most other APIs either allow any origin or reflect it; the marketplace API allows `localhost` but not `127.0.0.1`.
- **Display:** the canvas fills the browser window. On web the desktop 1280×720 window resize is skipped, and the canvas is capped at about 1920×1080 device pixels on high-DPI screens (`?maxpixels=N`, or `?maxpixels=native` to turn the cap off).
- **Known cosmetics:** a 404 for `sentry-bundle.js` (the Sentry addon's web file isn't exported), `set_thread_safety_checks_enabled` "forbidden on main thread" log spam, and `--emulate-ios` calling a missing iOS plugin.

## Risks

1. **Main-thread CPU budget.** GLTF loading, CRDT processing, physics and avatars share one thread on web. Genesis Plaza alone was fine. Many scenes plus a crowd on a mid-range laptop or phone is the real test.
2. **Memory.** The wasm32 limit is 4 GB, and mobile browsers allow much less. The in-memory content cache has to go first.
3. **Download.** About 52 MB of brotli assets plus about 12 MB of code. Needs asset trimming, streaming, and a release/LTO template.
4. **Driver ownership.** We'd maintain a large AI-written driver and patched Tint, and keep up with browser WebGPU changes (that's how today's regressions happened).
5. **Toolchain hacks.** Nightly Rust with build-std, a one-line emsdk patch (an export-name check that rejects Rust side modules), and our own web templates. The stock editor doesn't emit `renderingDriver`; the export script injects it.

## Reproduce

See [README.md](README.md) in this folder.
