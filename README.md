# Godot Web Preview

Browser runtime for previewing Decentraland scenes with the mobile client inside a
web editor. This public repository contains the Godot engine, the mobile explorer,
and the browser changes needed to build them together for WebGPU.

The distributable is `@dcl-regenesislabs/mobile-preview-runtime`: prebuilt engine,
client and assets. An editor installs an exact npm version and supplies the scene
server, asset relay and preview controls. It does not build Godot or Rust.
The first npm release is being prepared as **0.1.0**; it is not published yet.

## What the preview supports

- Embedded or detached preview with mobile controls: click scene UI directly,
  drag the world to rotate the camera, and use the joystick to move.
- A parcel-sized grass floor, without surrounding islands or decorative blades.
- Pause/resume without restarting the runtime. Closing releases the instance;
  minimizing keeps its memory allocated.
- Scene UI, scene messages, MessageBus and authoritative state through browser
  LiveKit. Pulse uses WebTransport for player presence and movement.
- Gameplay APIs including `movePlayerTo`, emotes, teleport/realm confirmation
  dialogs, scene events, world time and locale.

The consuming editor owns screenshots, hot reload, embedding and tab controls.
It must keep the scene server running while the preview is open. The runtime uses
its own guest identity; editor wallet credentials are not passed to it.

This is a development preview, not the full mobile app. Voice, video, wallet
operations, portable experiences and file-reading APIs are excluded. Movement
interpolation and full world multiplayer remain unsupported. Chrome on Apple
Silicon is the tested interactive environment; Linux Chromium checks graphics
regressions in CI. Native phone behavior and sustained memory/CPU use still need
device testing. Frame rate and startup time depend on the scene and hardware.

## Sources and upstream updates

| Path | Purpose |
| --- | --- |
| `godotengine/` | Godot 4.6.2 with the WebGPU driver and browser fixes |
| `godot-explorer/` | Decentraland's mobile explorer with browser runtime changes |
| `upstreams.json` | Canonical upstream repositories and imported revisions |
| `emsdk/` | Public Emscripten submodule, initialized by the build |
| `packages/runtime/` | npm packaging and artifact integrity checks |
| `tests/` | Graphics, input, gameplay, networking and updater regressions |
| `build.sh` | Engine, native importer, WebAssembly and export commands |

**The engine and explorer are Git subtrees.** They appear as normal folders because
all their files are committed here. Unlike submodules, they do not require a
separate checkout or get a special repository link in GitHub. Subtree import
commits preserve the upstream merge history; browser changes are separate commits.

The daily updater tracks `decentraland/godot-explorer` on `main` and the matching
engine revision on `decentraland/godotengine`'s `4.6.2` branch. It opens a draft PR
and starts validation when updates are available. It never merges or releases.
Conflicts and engine-version changes require review.

```sh
git switch -c chore/update-mobile-upstreams
node scripts/upstream.mjs check
node scripts/upstream.mjs sync
```

Merge source-update PRs with a **merge commit** so the next update retains subtree
metadata. See [upstream updates and conflict handling](docs/UPSTREAMS.md).

## Build locally

The build is tested on macOS with Apple Silicon. Install Xcode Command Line Tools,
[Rust via rustup](https://rustup.rs), Python 3.10+, SCons and **Node.js 24+**.
Use Chrome with WebGPU available. Allow substantial disk space and time for the
first engine and Rust builds; they run with two jobs at background priority.

```sh
git clone https://github.com/dcl-regenesislabs/godot-web-preview.git
cd godot-web-preview
python3 -m pip install scons==4.11.1
./build.sh emsdk engine native wasm preview
```

The build downloads Emscripten 4.0.11, the pinned Rust toolchains and the native
Godot importer. It writes the preview to `godot-explorer/build/web/`. The engine
uses WebGPU with a single-threaded WebAssembly runtime; scene JavaScript runs in
Web Workers. Native deno/V8, voice and video dependencies are excluded from the
browser build. The WebGPU driver originates from
[dwalter/godotwebgpu](https://github.com/dwalter/godotwebgpu), with fixes included here.

Pass the export's absolute path as `MOBILE_PREVIEW_EXPORT` to the editor's dev stack.
The `preview` export leaves launch arguments to the host editor. For a standalone
guest explorer after the initial build, use:

```sh
./build.sh export
node godot-explorer/web/tools/serve-web.mjs godot-explorer/build/web 8060
```

Open `http://localhost:8060/` in Chrome. The standalone export uses the realm and
location defaults in `build.sh`; `EXPLORER_ARGS` overrides them. Use `preview` again
before packaging an editor runtime.

| Changed files | Rebuild |
| --- | --- |
| Engine C++ | `./build.sh engine preview` |
| Explorer Rust or scene-worker JavaScript | `./build.sh wasm preview` |
| GDScript, HTML or assets | `./build.sh preview` |

Do not commit generated binaries. See [developer build notes](CLAUDE.md) for
compiler constraints and [WebGPU porting lessons](docs/WEBGPU-PORTING.md) for
confirmed rendering fixes, memory limits, host integration and scene behavior.
The [original browser report](godot-explorer/web/REPORT.md) preserves historical
measurements from the first spike.

## Validate

Run the source and package tests:

```sh
node --test tests/*.test.mjs packages/runtime/*.test.mjs godot-explorer/godot/web/preview.test.mjs godot-explorer/web/tools/{scene-comms,scene-gameplay,pulse}.test.mjs
node godotengine/webgpu_tests/driver_unit_tests/run_tests.mjs
```

After building, export and run a graphics regression:

```sh
./build.sh test webgpu-buffer-coherence
npm ci --prefix godot-explorer/web/tools
npx --prefix godot-explorer/web/tools playwright install chromium
node godot-explorer/web/tools/run-web.mjs tests/webgpu-buffer-coherence/build 90 buffer.png --assert=bufferCoherenceResult
```

The [scene UI fixture](tests/scene-ui-input/README.md),
[gameplay fixture](tests/scene-gameplay/README.md) and
[scene-server fixture](tests/scene-comms/README.md) provide integration checks.
In the editor, verify first-click UI actions, camera dragging, joystick movement,
minimize/restore, screenshots, detaching and scene reload. Only one preview instance
should run at a time.

CI runs unit tests and dependency audits first, then builds the engine and Rust client
in parallel on separate three-core macOS 15 runners.
Dedicated runners use normal CPU priority; local builds keep their two-job,
background defaults. Export runs on macOS after both builds and the native
gameplay/Pulse checks pass. Texture, buffer and frame-pacing
regressions run in Linux Chromium with SwiftShader before package validation. Linux jobs
use Ubuntu 24.04 explicitly so `ubuntu-latest` migrations do not change the build.

The native Rust library is a temporary export tool: the headless Godot editor
loads its custom classes when importing assets and creating the `.pck`. Only the
web runtime is packaged for npm. Native Deno compatibility is outside the normal
web release path; upstream syncs enable it, and manual runs can opt in with the
`native-compatibility` input.

The engine ZIP is cached by source tree and toolchain inputs and saved immediately
after compilation. Emscripten installations and Cargo dependencies are cached too;
native dependencies use a separate cache when the macOS runner changes.
Cargo caches are saved only after a successful job so a cancelled build cannot
leave an incomplete cache under the final key. Rust still rebuilds the workspace
crates. Documentation-only changes skip
compilation and packaging; manual runs and release tags always run the full
pipeline. New commits cancel obsolete validation runs, while release runs finish.
Cold and cached build times differ; use the separate Actions steps to measure both.
Use `validate` as the required branch check: it accepts documentation-only runs
and requires every build, audit and regression job for runtime changes.

The npm tarball is the only retained runtime export (14 days); its `package/dist/`
contains the web files. Intermediate engine, importer and regression artifacts
expire after two days; regression screenshots and failure logs remain for seven days.
Only use tarballs from runs where `validate` passed.
Passing software-rendered graphics checks does not establish device performance.

## npm releases

Releases use [Decentraland's Oddish Action](https://github.com/decentraland/oddish-action)
with npm OIDC trusted publishing and provenance. No npm token or
`GODOT_SOURCE_TOKEN` is required. Editors consume the package; only this repository
owns the runtime build and release workflow.

The release tag must match `packages/runtime/package.json`: **`0.1.0`, without a
`v` prefix**, for the first release. After build, audit and browser checks pass,
the `production` job verifies the packaged assets and lets Oddish publish to
`latest`. Main pushes, pull requests and manual validation runs do not publish.

The npm package must first be bootstrapped and its trusted publisher configured
for this repository, `preview.yml` and environment `production`. Configure release
reviewers before publishing. See [package and OIDC setup](packages/runtime/README.md)
and [migration and remaining setup](docs/MIGRATION.md).
