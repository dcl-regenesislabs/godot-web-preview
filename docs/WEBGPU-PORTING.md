# WebGPU porting lessons

Durable findings from the October 12–15, 2026 browser port and editor-preview
sessions, checked against the implementation at `4d8b3a8b`. This covers rendering,
runtime behavior and host integration; build/release operations live in
[CLAUDE.md](../CLAUDE.md) and [upstream maintenance](UPSTREAMS.md).

The [original spike report](../godot-explorer/web/REPORT.md) records the first
successful world load. Its sizes, performance numbers, gaps and checkout
instructions are historical. Use this guide and the root README for current
behavior. Host-editor findings below describe the integration contract; the
editor adapter is maintained outside this repository.

## Architecture and build constraints

The Godot WebGPU template owns rendering, physics, asset import and the Rust
`dclgodot` GDExtension on the browser main thread. One Web Worker per scene runs
the existing SDK JavaScript loader through a `Deno.core.ops` compatibility layer.
Workers exchange CRDT frames and API requests with the Rust host; they do not
contain a second Godot engine or the native Deno runtime.

Relevant code: [scene host](../godot-explorer/lib/src/web/scene_host.rs),
[scene Worker](../godot-explorer/lib/src/web/scene_worker.js),
[Worker bridge](../godot-explorer/lib/web-scene),
[Tokio pump](../godot-explorer/lib/src/scene_runner/tokio_runtime.rs).

- Keep `threads=no` and Forward Mobile. The driver assumes main-thread WebGPU
  calls. Enabling threads requires a separate design and browser-isolation review.
- The browser Tokio runtime is driven by Godot frames. Native thread, socket and
  filesystem assumptions need browser implementations: [fetch](../godot-explorer/lib/web-reqwest),
  [WebSocket](../godot-explorer/lib/web-websocket), MEMFS and the vendored
  [RPC dependency](../godot-explorer/lib/vendor/rpc-rust). Browsers cannot use the
  native UDP/SNTP path.
- Emscripten 4.0.11 needs the checked-in side-module export patch. Wasm Rust uses
  nightly with `-Zbuild-std=std,panic_abort` because the template has no C++ exception
  support. Use `build.sh` to select the pinned toolchains.
- gdext's prebuilt bindings are 64-bit. [wasm-env.sh](../godot-explorer/web/wasm-env.sh)
  selects the Cargo registry's **4.5 API JSON** for 32-bit generation. Substituting
  an API dump from the 4.6.2 editor broke code generation during the port.
- Each `EM_JS` bridge function needs its export in
  [lib/.cargo/config.toml](../godot-explorer/lib/.cargo/config.toml).
  `-Zlink-native-libraries=no` discards the C archives containing those functions.
- The native extension is required by the headless Godot importer to instantiate
  custom classes while exporting assets. It is a build tool, not a shipped browser
  dependency. Keep the native and wasm implementations compiling.
- Give every checkout its own Cargo target directory. Sharing or cloning a target
  directory reused another checkout's compiled source during debugging. Share a
  compiler cache instead. Local builds remain two jobs at background priority.

## Rendering: distinguish shader, upload and readback failures

A successful optimized world load did not prove local-preview rendering. Raw
GLTF imports and wearables take different material and serialization paths.
Likewise, zero WebGPU validation errors did not mean textures survived serialization.

| Symptom | Confirmed cause and retained fix | Where to look |
| --- | --- | --- |
| White materials or shader conversion errors | Tint rebuilt arrays without preserving explicit stride. Leave unchanged array types intact when rewriting atomics. | [Tint transform](../godotengine/thirdparty/tint/src/tint/lang/wgsl/ir/atomic_vec2u_to_from_u64.cc) |
| Invalid shadow sampling pipelines | PCSS depth reads used a filtering sampler. The driver's WGSL pass rewrites the affected reads to `textureLoad`. | [WebGPU driver](../godotengine/drivers/webgpu/rendering_device_driver_webgpu.cpp) |
| Compressed color textures fail validation | BC7/ASTC needed matching linear/sRGB view formats. | [WebGPU driver](../godotengine/drivers/webgpu/rendering_device_driver_webgpu.cpp) |
| Terrain/material shaders exceed device limits | Default limits were insufficient: some shaders need 17 inter-stage variables and the mobile renderer needs more than eight storage buffers. Request supported adapter limits in both device-creation paths. | [Engine loader](../godotengine/platform/web/js/engine/engine.js), [preview shell](../godot-explorer/godot/web/preview.html) |
| Runtime-imported materials become magenta | The native import optimization switches to a per-vertex shader that fails WebGPU translation. Keep the wasm import path on per-pixel lighting. | [GLTF import](../godot-explorer/lib/src/content/gltf/common.rs) |
| Saved/reloaded GLTF loses textures without GPU errors | `ImageTexture.get_image()` ran before asynchronous GPU readback completed, so the saved `.scn` lacked pixels. Retain independent CPU images for uploaded 2D textures and return copies for synchronous reads. | [Texture storage](../godotengine/servers/rendering/renderer_rd/storage_rd/texture_storage.cpp), [round-trip regression](../tests/webgpu-texture-roundtrip/README.md) |
| Mesh/collider data reads as zero or stale | Godot needs synchronous vertex/index reads. Keep CPU mirrors coherent across uploads, updates, clears and copies; invalidate them when writable GPU use makes them stale. | [WebGPU driver](../godotengine/drivers/webgpu/rendering_device_driver_webgpu.cpp), [buffer regression](../tests/webgpu-buffer-coherence/README.md) |

Do not remove CPU mirrors just to lower memory. The buffer test covers repeated
reads, read-only compute use and a writable compute pass; the latter must eventually
return GPU results. Texture snapshots must survive source-image mutation, updates,
replacement and proxy reads without aliasing caller-owned images. They are not a
general synchronous readback solution for render targets.

Texture formats are another distinct issue: WebGPU has no component swizzle, so
L8/LA8 need RGBA conversion. The texture regression includes RGB/RGBA/grayscale and
mipmaps. An early RGB8 conversion experiment was incorrectly declared the complete
fix; magenta persisted until the import/serialization problems were isolated.
Changing compression or disabling mipmaps also failed to explain every symptom.

For a new failure, keep the shader name, validation error and
`WEBGPU_FAILED_SPIRV_B64` dump, then replay it with the
[offline Tint tool](../godotengine/drivers/webgpu/tint_cli/build.sh). Compare raw
imports with preoptimized assets. After a serialization fix, reimport previously
cached `.scn` files; rebuilding the engine cannot restore their missing pixels.
Inspect source images before attributing a color to the driver: the old empty
parcel atlas really contained red terrain.

The early directional-light-array diagnosis was a hypothesis, not a confirmed
root cause. A scene UBO mismatch was still reported in the sky/glow smoke test;
keep that as an open investigation rather than claiming every shader variant works.

## Scene loading and a smaller local preview

`--local-editor-preview` gates browser-preview behavior; it must not change native
mobile behavior. The [local runtime](../godot-explorer/godot/src/logic/local_preview_runtime.gd)
and [startup](../godot-explorer/godot/src/global.gd) omit unrelated services and UI.
The shell forces mobile input and a 1600×720 canvas at DPR 1. Desktop/mobile
emulation flags are not interchangeable; emulating iOS previously invoked a
native plugin that was unavailable on web.

Two missing-scene bugs were independent of graphics:

- **Authored entities missing, scripted entities visible:** `main.crdt` downloaded
  correctly, but its initial dirty components were not sent to the renderer before
  Worker frames consumed them. `scene_host::spawn` now flushes them immediately,
  including after hot reload. Test an authored GLTF and a script-created primitive,
  not just one of them.
- **Preview stuck loading after removing islands:** terrain removal also bypassed
  loading-session creation. [Scene fetching](../godot-explorer/godot/src/logic/scene_fetcher.gd)
  must still start the loading session and wait for scene/model readiness. Engine
  startup alone is not the signal that a scene is ready to capture.

The default local ground is a cheap parcel-sized mesh using a
[preview grass material](../godot-explorer/godot/assets/empty-scenes/local_preview_floor_material.tres),
managed by [BaseFloorManager](../godot-explorer/godot/src/logic/base_floor_manager.gd).
It sits just below scene ground. Surrounding islands and decorative grass blades
are unnecessary for this preview. This is client-provided ground; it does not add
an asset to the scene or guarantee identical terrain in every deployment realm.

## Hosting, isolation and hot reload

The final host integration serves the runtime on a separate origin and relays only
scene metadata/content. An earlier same-origin service-worker experiment proved
content access, but was not the final security boundary. An editor owns its scene
server, relay, window controls and screenshot adapter; the npm package owns the
prebuilt engine, client and assets. Keep Godot integration out of a general-purpose
Bevy explorer host.

Requirements learned while embedding:

- The tested host uses `Document-Isolation-Policy: isolate-and-credentialless` for
  the preview and a matching host policy. Adding COOP/COEP only to a nested frame
  left the tested document non-isolated and GPU adapter creation stalled. Check
  `crossOriginIsolated` in the actual iframe. The standalone loopback test server
  does not establish the editor's embedding policy.
- Scope `postMessage` to the expected origin, source window, session and instance.
  Relay only the required `/about`, `/scene.json`, content entities and content
  files, with bounded request sizes, concurrency and timeouts. Do not relay storage,
  environment or arbitrary service routes, credentials or redirects.
- Support **HEAD as well as GET on `/about`**. Godot's reachability probe otherwise
  gets a 404 and reports a disconnected realm even when asset GETs work.
- Keep the scene server running while the preview is open or detached. Opening the
  runtime does not create an authoritative server by itself.
- A detached preview uses a host-owned `about:blank` shell containing the runtime
  iframe. Directly navigating the popup to the isolated runtime severed the opener
  relationship. Moving between modes must leave only one live preview instance.
- Keep editor inspector/assistant toggles reachable while the preview is open;
  the preview should occupy the editing area without trapping access to sidebars.

Hot reload requires the preview deep link:
`--fake-deeplink decentraland://open?preview=<realm-url>`. In the tested flow,
`--realm ... --preview` alone did not attach the preview listener. The
[HTML shell](../godot-explorer/godot/web/preview.html) intercepts only the matching
realm WebSocket and turns `scene-updated` into `SCENE_UPDATE`; unrelated sockets
must retain their native behavior. The host adapter transports these messages
across origins; a BroadcastChannel alone cannot do that.

Verify reload with a changed scene entity/content fetch, replacement scene Worker
and unchanged page `performance.timeOrigin`. A full page refresh repeats engine,
WASM and PCK initialization and is not a hot reload. Keep the dedicated HTML shell:
putting its JavaScript into the export preset's `head_include` lost backslashes and
silently broke a URL-matching regex during the port.

## Input, pause and visual validation

Desktop pointer capture consumed clicks intended for scene UI. Disabling capture
alone then removed camera control. The retained solution forces the mobile input
path: a visible cursor can click UI, dragging the world rotates the camera, and
the joystick moves the avatar. Test all three together, including Escape and the
first click after startup.

There was also a separate timestamp bug: the first press used zero, which the SDK
discarded as older than its initial input watermark. The
[event timestamp counter](../godot-explorer/lib/src/scene_runner/scene_manager.rs)
starts at one. The [UI fixture](../tests/scene-ui-input/README.md) proves both
`onMouseDown` and `onMouseUp`; a release-only or repeatedly clicked button could
hide the bug.

Minimizing must pause the Godot tree, disable rendering, release held actions and
stop scene-frame advancement. Host visibility and `document.hidden` are combined;
a browser visibility change must not resume a preview the user minimized. Restore
the previous pause/FPS state. Closing tears down the instance; minimizing preserves
its allocations to avoid a cold start.

Lowering the paused FPS cap originally made editor responsiveness worse: native
frame delay blocked the browser main thread. [OS_Web::add_frame_delay](../godotengine/platform/web/os_web.cpp)
now skips that delay for the single-threaded build; [web_main.cpp](../godotengine/platform/web/web_main.cpp)
already skips engine frames while yielding browser callbacks. The
[frame-pacing fixture](../tests/webgpu-frame-pacing/README.md) separately counts
engine frames and browser animation frames. A 5 FPS engine should not force the
surrounding editor to 5 FPS.

Screenshot lessons for an editor or assistant:

- Capture the **mobile runtime canvas**, including its touch controls and scene
  UI. A picture of the editing engine, or a sketch based on scene code, is not
  evidence of mobile layout.
- Wait for scene readiness; temporarily restore a minimized preview for capture
  and restore its state afterward unless the user changed it meanwhile. Propagate
  cancellation and loading errors instead of presenting a stale image.
- The host's capture hook copies the canvas after a WebGPU queue submission. A
  plain `requestAnimationFrame` capture sometimes returned a cleared black JPEG.
  The package shell's basic capture fallback still uses rAF, so it is not equivalent
  to that host hook.
- Covered/occluded browser windows stalled frames and screenshots during testing.
  That was initially misdiagnosed as two engines being unable to coexist. Measure
  frame advancement and test the actual same-tab layout; two separate visible
  windows do not prove embedded behavior.
- Automation that disables background throttling or emulates focus can mask native
  visibility behavior. Repeat minimize/restore in an ordinary foreground browser.
  Use the workspace's persistent test-browser harness when available; do not launch
  Electron or modify a personal browser profile to debug the web preview.

## Memory and startup measurements

The local-preview [cache setting](../godot-explorer/godot/src/config/general_settings.gd)
targets **128 MiB** of content files, and the shell disables persistent-path
restoration. MEMFS consumes RAM: restoring a native-sized disk cache from IDBFS
would pull it back into memory. This target is not a total-memory ceiling;
[cache eviction](../godot-explorer/lib/src/content/resource_provider.rs) can also
admit a file larger than its target after evicting everything else. Download/import
temporaries can peak above the retained-cache size.

WASM linear memory retains its high-water allocation even when resources are
freed internally. PCK contents, imported resources, CPU texture/buffer mirrors,
GPU textures and scene Workers add different costs. Pause reduces work, not those
allocations. Repeatedly closing and opening trades memory release for cold startup;
reuse a paused instance when the creator intends to return soon.

Measure first open, steady state, hidden state, repeated scene reloads and final
close separately. Record the scene, browser, hardware, renderer, canvas size and
build type. Browser memory attribution can count the same shared WASM allocation
against multiple Workers; do not sum those entries or charge an entire shared GPU
process to one preview. Compare process memory and known heap/buffer sizes.

Early one-scene measurements around 0.9–1 GiB, cold starts around 15 seconds and
30 FPS on an M4 Pro were debugging observations, not budgets or guarantees for the
current package. Several successful reloads are not a long-duration leak test.
Software-rendered Chromium proves regression behavior, not hardware performance
or mobile-browser memory safety.

## Scene messages, Pulse and gameplay APIs

These are separate paths. Validate each rather than treating a connected socket
or a resolved SDK promise as proof of scene behavior.

| Path | Implementation and purpose | Proof |
| --- | --- | --- |
| Scene communication | [Browser LiveKit](../godot-explorer/web/scene-comms.js) and [adapter lookup](../godot-explorer/lib/src/web/scene_comms.rs) carry binary SDK messages, MessageBus strings, targeted replies and authoritative CRDT using the preview's own guest identity. | [Scene-server fixture](../tests/scene-comms/README.md): actual server receipt, targeted reply and synchronized counter. |
| Player presence/movement | [Pulse WebTransport](../godot-explorer/web/pulse.js) with the [wasm transport](../godot-explorer/lib/src/comms/pulse/wasm.rs) uses the shared protocol/auth/decoder. Native keeps ENet. Local preview enables Pulse unless `--no-pulse` is supplied. | The same fixture sends **no position in its ping**; the server must observe `PlayerIdentityData` and changing `Transform`, including proximity to the red cube. |
| Gameplay actions/events | [Typed actions](../godot-explorer/lib/src/web/scene_actions.rs), [scene host](../godot-explorer/lib/src/web/scene_host.rs) and [shared events](../godot-explorer/lib/src/dcl/scene_events.rs) reuse renderer RPC handling and native checks. | [Gameplay fixture](../tests/scene-gameplay/README.md): actual movement, independent camera/avatar targets, blocked out-of-parcel movement, visible emotes and confirmation dialogs. |

Before these fixes, outgoing scene messages disappeared, movement could report
success without moving, and scene replies could work while the authoritative
server still could not see the player. Fix the owning path; do not compensate by
putting player coordinates into an unrelated application message.

Preserve scene identity when invoking renderer APIs. The action enum rejects
unknown fields, nonfinite movement targets and invalid emote masks; pending actions
are capped at 128 per scene frame and Worker scene events at 512. Unsupported ops
throw an explicit error instead of returning fake success. World time and locale
come from the client rather than permanent stub values.

Keep transport bounds and cleanup: LiveKit limits packets/queues; Pulse requires
HTTPS without URL credentials, bounds reliable frames to 4 KiB and its send queue
to 256 messages/256 KiB, and coalesces pending movement datagrams. Tests cover
fragmented frames, overflow, timeout and disconnect paths. Scene reload/close must
remove old Workers and connections; pause keeps connections while movement/frame
updates stop. Validate reconnect and detached mode too.

Opening and cancelling teleport/realm dialogs was validated; accepting real
navigation was not. Movement interpolation remains unsupported by the underlying
handler. Portable experiences, file reads, wallet operations and external URL/NFT
dialogs remain outside the browser-preview scope. Full world multiplayer, voice,
video, native phone behavior and peer/emote rendering need separate validation.

The Worker is **not a hardened sandbox**: it evaluates scene code with browser
globals, including fetch and WebSocket. Bounded bridge queues and a separate
preview origin reduce specific risks; they do not establish native permission
parity or protection against malicious scene CPU/memory use. Keep editor credentials
and privileged routes outside the preview. Do not describe this development
runtime as ready to execute arbitrary untrusted scenes safely.

## Before changing a previously fixed path

Run the relevant linked fixture, record the failing observation, and repeat after
the change. Use the root [validation commands](../README.md#validate) for unit and
browser checks. An export succeeding is not proof that a graphics test ran.

For host integration, cover first-click UI, drag camera, joystick, authored and
scripted entities, textured raw GLTF, floor, readiness, hot reload, screenshot,
minimize/restore, detach and final close. For multiplayer, keep the scene server
running and verify both message delivery and server-observed player state.
Record remaining failures explicitly; avoid promoting an early hypothesis or a
single screenshot into a general claim of compatibility or performance.
