# Decentraland explorer in the browser (WebGPU)

This repo runs the [Decentraland Godot explorer](https://github.com/decentraland/godot-explorer) inside a
web browser, with no install. It's the same explorer that ships on desktop and mobile, compiled for the web
and drawn with **WebGPU**, the browser's modern graphics API.

Today it boots, enters the world as a guest, and renders Genesis Plaza from its real scene code: buildings,
lights, avatars and the scene's own UI, at a steady 30 fps in Chrome on a Mac.

## Try it

```bash
git clone --recursive https://github.com/kuruk-mm/test-godot-web.git
cd test-godot-web
./build.sh
node godot-explorer/web/tools/serve-web.mjs godot-explorer/build/web 8060
```

Open **http://localhost:8060/** in Chrome. Loading takes a minute or two, then you appear at Genesis Plaza
as a guest.

### What you need

- **Access** to three private repos:
  - `kuruk-mm/test-godot-web` (this one)
  - `kuruk-mm/godot-explorer-test`
  - `kuruk-mm/dclgodotengine`

  The last two are pulled in automatically, so the clone fails without access to them.
- **A Mac** (tested on Apple Silicon). Linux will probably work but is untested; Windows isn't supported.
- **These tools:**
  - Xcode Command Line Tools: `xcode-select --install`
  - Rust: [rustup](https://rustup.rs). The build installs the exact versions it needs.
  - Python 3.10 or newer: `brew install python@3.13`. The Mac's own Python is too old.
  - SCons, Godot's build tool: `pip3 install scons`
  - Node 18+: `brew install node`
  - Google Chrome
- **About 25 GB of disk.**
- **Time for the first build:** it runs in the background on 2 cores, so the computer stays usable while it
  works. That takes up to an hour or two, mostly the engine. `JOBS=8 ./build.sh` is much faster but takes
  over the machine.

## How it works

The explorer has three parts, and each needed a browser version:

| Part | On desktop and mobile | In the browser |
|---|---|---|
| **Engine:** draws the world | Godot 4.6.2 drawing with Vulkan or Metal | The same Godot, compiled to WebAssembly, drawing with WebGPU |
| **Core:** content, avatars, networking, scenes | A Rust library | The same Rust library, compiled to WebAssembly, using the browser's networking |
| **Scenes:** each parcel's JavaScript | Runs inside the app in an embedded V8 | Runs in the browser's own JavaScript engine, in a background worker per scene |

**WebAssembly** is how browsers run code written in languages like C++ and Rust at near-native speed.
**WebGPU** gives that code access to the graphics card, much like Vulkan or Metal do.

Godot has no WebGPU support of its own. The driver comes from the community project
[dwalter/godotwebgpu](https://github.com/dwalter/godotwebgpu), with fixes on top in our engine fork.

## How the build works

The finished product is a folder of ordinary files, `godot-explorer/build/web/`, that any web server can
host:

| File | What it is |
|---|---|
| `index.html`, `index.js` | The page and the loader |
| `index.wasm`, `index.side.wasm` | The Godot engine |
| `dclgodot.wasm` | The explorer's Rust core |
| `scene-comms.js`, `livekit-client.umd.js` | Local-preview scene networking |
| `pulse.js` | Pulse WebTransport for player state |
| `index.pck` | The game content: scripts, scenes, UI, models, textures |

The browser opens the page and starts the engine. The engine loads the Rust core as a plugin and runs the
content.

`./build.sh` produces that folder in five steps:

1. **Install the compiler.** Downloads Emscripten, the toolchain that turns C++ and Rust into WebAssembly.
2. **Build the engine.** Compiles our Godot fork for the browser, with only the WebGPU renderer and plugin
   support. This is the slow step, about an hour.
3. **Install the editor.** Downloads the Godot editor the explorer uses and builds the Rust core for the Mac.
   The editor needs that build only to open the project.
4. **Build the Rust core for the browser.** Parts that can't run in a browser are turned off: the embedded
   JavaScript engine, voice chat and native video. Browser versions replace them where needed.
5. **Package it.** The editor combines the engine, the Rust core and the content into `build/web/`.

Shaders convert to WGSL in the browser. The build disables optional shader precompilation
with `webgpu_precompile_shaders=no`, so it needs neither a native Tint tool nor glslang.

After a change, re-run only the steps it affects:

| You changed | Run |
|---|---|
| The engine (`godotengine/`) | `./build.sh engine export` |
| The Rust core or scene runtime (`godot-explorer/lib/`) | `./build.sh wasm export` |
| Scripts, scenes or assets (`godot-explorer/godot/`) | `./build.sh export` |

## What works and what doesn't yet

**Works:**
- Loading the world.
- 3D rendering with lights, shadows and materials.
- Avatars.
- The HUD and UI.
- Scene scripts, including their own UI, player data and signed requests to their servers.
- Guest login.

**Not yet:**
- Clicking or interacting with scene objects.
- Full world multiplayer and voice chat (local preview scene messages and Pulse player state are supported).
- Wallet sign-in.
- Video.
- Some in-world banners, blocked by server settings (CORS).

The full explorer has been tested in Chrome on a Mac. Graphics regressions also run
in Linux Chromium with SwiftShader; that does not establish hardware performance on Linux.

## What's in this repo

| Path | What |
|---|---|
| `godot-explorer/` | The explorer, with the browser-specific code added |
| `godotengine/` | Our Godot engine fork with the WebGPU driver |
| `emsdk/` | The Emscripten compiler (downloaded and set up by `build.sh`) |
| `tests/` | Small WebGPU test scenes, useful when working on the engine |
| `build.sh` | The whole build in one script |

More detail:
- [godot-explorer/web/REPORT.md](godot-explorer/web/REPORT.md): findings, sizes, risks and next steps.
- [CLAUDE.md](CLAUDE.md): working notes for developers and AI assistants.

## Local scene editor preview

The editor preview runs the same mobile renderer with account panels, remote polling,
telemetry and surrounding islands disabled. Minimizing pauses scene processing and
rendering; restoring retains the running instance. Closing releases it. The default grass
floor covers the scene's parcels with two triangles per parcel in one shared mesh;
surrounding islands and grass blades remain disabled.

The local preview uses mobile input: click buttons directly and drag the world to
look around. It keeps the mouse cursor free, including after Escape or closing a
panel; desktop pointer lock is disabled for mobile mode.

Local preview scenes connect to their scene room through the bundled browser LiveKit
client. Binary SDK messages, authoritative state synchronization and MessageBus
messages use the original preview scene ID. The runtime signs its own guest
connection; editor wallet credentials are not relayed. Voice and remote media are
disabled. Queues are bounded while paused or disconnected, and unloading a scene
closes its room. The editor must keep its scene server running.

Pulse uses browser WebTransport on port 7743 for player presence, movement and
emotes, sharing the native client's authentication and protocol. Local preview
launches enable it explicitly and derive the same `lsd:` realm as the scene
server from the scene entity ID. `--no-pulse` opts out; `--pulse-server=host:port`
overrides the endpoint and requires a browser-trusted TLS certificate. Reliable
queues are bounded and pending movement is coalesced to the latest sample. A
closed preview releases its connection; minimizing pauses gameplay updates.

Use [the scene-server regression](tests/scene-comms) to check an actual server
request/reply and synchronized state.

Gameplay APIs use the native renderer handlers: `movePlayerTo` (including separate
camera/avatar targets), predefined/custom emotes, stopping emotes, teleport and
realm-change confirmation dialogs. Scene event subscriptions, world time and locale
come from the running client. The existing scene boundary checks still apply.
Use [the gameplay regression](tests/scene-gameplay) to verify these in the preview.
Movement interpolation, portable experiences, file reads, wallet operations and
external URL/NFT dialogs remain unsupported by the browser runtime.

After the initial toolchain and native setup, build the preview with:

```bash
./build.sh engine wasm preview
```

For GDScript or HTML changes, `./build.sh preview` is enough. Unlike `export`, this
leaves launch arguments to the editor. The output is `godot-explorer/build/web/`.
The export command fails if Godot fails; it never reports an older export as a new build.

This repository publishes `@dcl-regenesislabs/mobile-preview-runtime` to npm using OIDC.
The editor installs an exact released version. For local runtime development, pass the
export's absolute path as `MOBILE_PREVIEW_EXPORT` to the editor dev stack.
The editor owns runtime hosting, screenshots, hot reload and the scene resource relay.
This workspace owns the engine, mobile runtime source and their build toolchain.
See [npm packaging and release setup](packages/runtime/README.md).
Compiled output is ignored by Git; packaging and publishing are separate steps. The local
server streams files and rejects paths or symlinks that escape the export directory.

Regression checks:

```bash
node --test tests/*.test.mjs packages/runtime/*.test.mjs godot-explorer/godot/web/preview.test.mjs godot-explorer/web/tools/{scene-comms,scene-gameplay,pulse}.test.mjs
./build.sh test webgpu-frame-pacing
./build.sh test webgpu-texture-roundtrip
./build.sh test webgpu-buffer-coherence
```

Serve each exported fixture as its README describes and check its browser PASS result.
In the editor, open **Preview → As a phone**, minimize/restore, capture the scene, then
move it to a tab and rebuild the scene. Only one mobile runtime should run. Preview
memory remains allocated while minimized; closing is necessary to release it.
For the input regression, click the world, press Escape, then click a UI button.
The first click must activate it without locking the pointer. Close the panel and
repeat; drag the world to rotate the camera and drag the joystick to move. Check
these steps both embedded and in a detached preview.
Use [the scene UI fixture](tests/scene-ui-input/README.md) to check actual SDK UI
callbacks, including the first press after startup, separately from client HUD buttons.

## Continuous integration

The `Mobile preview` workflow runs on pull requests and main pushes. It builds on macOS
from the exact submodule commits, runs the protocol/export/server checks, and uploads the
web export and graphics fixtures. A Linux job runs Chromium under Xvfb with Mesa and
SwiftShader, requiring explicit PASS results for texture serialization, buffer coherence
and frame pacing. It also requires a WebGPU renderer, completed GPU work and no engine/browser/GPU
errors. Screenshots remain available as workflow artifacts. Hardware performance
and native phone behavior still need device testing.

The browser's Rust dependencies are checked against a freshly downloaded public OSV database
without uploading the dependency inventory. Three maintenance-only notices are documented in
`.github/osv-scanner.toml` and expire on November 15, 2026. Security findings fail the workflow;
every source CI build runs this check before the package can be published.

Add a repository secret named `GODOT_SOURCE_TOKEN` with read-only Contents access to this
repository and both private source submodules. Checkout does not persist the credential.
Push the source submodule branches first, then the parent branch with their committed
pointers; the source commits must be fetchable before parent CI can run. After the build,
offline audit and browser assertions pass, CI prepares the npm tarball. Matching stable
`vX.Y.Z` tags publish to npm with OIDC through the `production` environment; all other runs
only validate. Consumers pin an npm version and need no private source token. Nothing
deploys an application. See [trusted publisher setup](packages/runtime/README.md).

For a local browser assertion after exporting a fixture:

```bash
npm ci --prefix godot-explorer/web/tools
node godot-explorer/web/tools/run-web.mjs tests/webgpu-buffer-coherence/build 90 buffer.png --assert=bufferCoherenceResult
```

The buffer fixture checks initial data, immediate CPU updates, clears/copies, and a buffer
used first by a read-only compute shader and then by a writable shader. Before the
coherence fix it reports stale data after both CPU updates and GPU writes.
