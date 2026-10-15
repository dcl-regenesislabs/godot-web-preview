# Decentraland Godot Explorer

[![codecov](https://codecov.io/gh/decentraland/godot-explorer/branch/main/graph/badge.svg)](https://codecov.io/gh/decentraland/godot-explorer)
[![CI](https://github.com/decentraland/godot-explorer/actions/workflows/ci.yml/badge.svg)](https://github.com/decentraland/godot-explorer/actions)
[![Android](https://github.com/decentraland/godot-explorer/actions/workflows/android_builds.yml/badge.svg)](https://github.com/decentraland/godot-explorer/actions)

Decentraland Godot Explorer is a cross-platform metaverse client built with Godot 4.6.2 and Rust, supporting desktop, mobile, and VR platforms.

The web shell's `--local-editor-preview` mode uses mobile touch input. Mouse clicks
tap UI controls and dragging the world rotates the camera without pointer lock.
The ordinary desktop client retains its mouse capture behavior.
Local preview scenes use a bundled browser LiveKit client for SDK messages and
authoritative state synchronization with the running scene server. The connection
uses the preview guest identity, excludes voice/media, and closes on scene unload.
Browser gameplay calls use the native handlers for player movement, camera/avatar
targets, emotes, stopping emotes and teleport/realm confirmation dialogs. Event
subscriptions share the native runtime's event processing; world time and locale
come from the client. Movement interpolation, portable experiences, file reads,
wallet operations and external URL/NFT dialogs remain unsupported in the browser.
The local preview also enables Pulse over WebTransport for player presence,
movement and emotes. Its signed guest joins the preview scene's `lsd:` realm.
`--no-pulse` disables it; the web endpoint defaults to port 7743 (native ENet uses 7777).

## ✨ Features

- **Cross-Platform**: Native support for Linux, Windows, macOS, Android, iOS, and VR
- **High Performance**: Rust core with Godot rendering engine
- **Decentraland SDK7**: Full compatibility with Decentraland scenes
- **Voice Chat**: Integrated spatial audio via LiveKit WebRTC
- **Web3 Integration**: Ethereum wallet support for NFTs and transactions
- **Developer Friendly**: Hot reload, comprehensive testing, and debugging tools

## 🚀 Quick Start

1. **Prerequisites**:
   - [Rust 1.90](https://www.rust-lang.org/tools/install)
   - Git
   - Platform-specific dependencies (see below)

2. **Clone and setup**:
   ```bash
   git clone https://github.com/decentraland/godot-explorer
   cd godot-explorer
   # Windows users: use a short path like C:/gexplorer
   
   # Check system health
   cargo run -- doctor
   
   # Install Godot and build tools
   cargo run -- install --targets <your-platform>
   ```

3. **Run the project**:
   ```bash
   # Run the client (builds automatically)
   cargo run -- run
   
   # Or run the editor
   cargo run -- run -e
   ```

## 📦 Platform-Specific Dependencies

### Linux (Ubuntu/Debian)
```bash
sudo apt-get update && sudo apt-get install -y \
  libasound2-dev libudev-dev \
  clang curl pkg-config \
  libssl-dev libx11-dev libgl1-mesa-dev libxext-dev
```

### macOS
```bash
brew install pkg-config
```

### Android Development
- Android SDK with NDK 28.1.13356709
- Set `ANDROID_SDK` or `ANDROID_HOME` environment variable
- Run: `rustup target add aarch64-linux-android`

## 📋 Quick Reference

| Command | Description |
|---------|-------------|
| `cargo run -- doctor` | Check system health and dependencies |
| `cargo run -- install` | Install Godot and protoc |
| `cargo run -- install --targets ios` | Install iOS templates (auto-strips debug symbols) |
| `cargo run -- install --targets ios --no-strip` | Install iOS templates with debug symbols |
| `cargo run -- build` | Build for host platform |
| `cargo run -- run` | Build and run the client (iOS phone layout; `-- --emulate-android` for Android) |
| `cargo run -- run -e` | Build and run the editor |
| `cargo run -- export --target android` | Export Android APK |
| `cargo run -- strip-ios-templates` | Strip debug symbols from iOS templates |
| `cargo run -- clean-cache` | Clear downloaded template cache |

## 🛠️ Development Workflow

### Building and Running

```bash
# Build for host platform
cargo run -- build

# Run the client (builds automatically). Mobile is the product, so a desktop run always
# emulates a phone: --emulate-ios by default (phone window, safe areas, portrait/landscape),
# or Android with `-- --emulate-android`.
cargo run -- run
cargo run -- run -- --emulate-android

# Run the editor
cargo run -- run -e

# Run with specific features
cargo run -- run --no-default-features --features use_livekit,use_deno
```

### Android Development

```bash
# Install Android dependencies
cargo run -- install --targets android

# Build for Android
cargo run -- build --target android

# Export APK (automatically uses .bin/release.keystore)
cargo run -- export --target android --format apk --release

# Export AAB for Play Store
cargo run -- export --target android --format aab --release
```

### iOS Development (macOS only)

```bash
# Install iOS dependencies (strips debug symbols by default, saves ~1.9GB)
cargo run -- install --targets ios
rustup target add aarch64-apple-ios

# Build for iOS
cargo run -- build --target ios

# Export iOS app
cargo run -- export --target ios
```

#### iOS Template Debug Symbols

By default, iOS templates are **stripped of debug symbols** during installation to save disk space (~2.1GB → ~234MB). This is fine for most local development.

```bash
# Default install (strips debug symbols, smaller size)
cargo run -- install --targets ios

# Install WITH debug symbols (needed for Sentry crash symbolication)
cargo run -- install --targets ios --no-strip

# Strip already-installed templates manually
cargo run -- strip-ios-templates
```

> **Note**: Downloaded templates are cached. If you've already installed stripped templates and need debug symbols, clear the cache first:
> ```bash
> cargo run -- clean-cache
> cargo run -- install --targets ios --no-strip
> ```

#### Android APK Hosting & Mobile Distribution

Every Android CI build uploads the signed APK to the R2 mobile-artifacts bucket and
links it from the PR build-report comment (served at
`https://mobile-artifacts.dclregenesislabs.xyz/`).

The `build` label (or the legacy alias `build-ios`) triggers `mobile_distribute.yml`:
iOS → TestFlight, plus a Slack "🤖 Android Build Ready" notification once the commit's
APK is in R2. It does not rebuild Android or push to a store.

Progress is reported in two places that mirror each other: a live Slack root card (+ threaded
replies) and a single **📱 Mobile build pipeline** comment on the PR, kept current by every
leg via `.github/scripts/pr-card.py` — build number/version, per-platform state (iOS →
TestFlight, Android → APK/AAB links) and a collapsed timeline. The TestFlight result comes
from the `ios-wait` job, which watches the `godot-asc-deploy` run it dispatched (matched by
that workflow's `run-name`, `🍏 <branch> @ <sha>`) with the existing `ASC_DEPLOY_DISPATCH_TOKEN`
— the private repo holds no token for this one.

```bash
# On a PR: add the build label (iOS TestFlight + Android Slack notification)
gh pr edit --add-label "build"

# Manual trigger on main/release (GitHub Actions UI or gh CLI)
gh workflow run "📱 Mobile Build & Distribute (iOS TestFlight + Android APK)" --ref main
```

The label is automatically removed after the build is triggered on PRs.

## 🧪 Testing

```bash
# Run integration tests
cargo run -- run --itest

# Run scene tests
cargo run -- run --stest

# Generate test coverage
rustup component add llvm-tools-preview
cargo install grcov
cargo run -- coverage --dev

# Update Docker test snapshots from CI artifacts (requires gh CLI)
cargo run -- update-docker-snapshots

# Update coverage test snapshots from CI artifacts
cargo run -- update-coverage-snapshots

# Optionally specify a branch or run ID
cargo run -- update-docker-snapshots --branch main
cargo run -- update-docker-snapshots --run-id 21769476899
```

## 🎮 Supported Platforms

- **Desktop**: Linux, Windows, macOS
- **Mobile**: Android (API 29+), iOS
- **VR**: Quest (Meta), OpenXR compatible devices

## 📁 Project Structure

```
godot-explorer/
├── godot/              # Godot project files
│   ├── src/            # GDScript source code
│   └── project.godot   # Project configuration
├── lib/                # Rust library
│   ├── src/            # Rust source code
│   └── Cargo.toml      # Rust dependencies
├── src/                # Build system (xtask)
└── exports/            # Build outputs
```

## 🐳 Docker Support

For CI/CD or consistent build environments:

```bash
# Linux/Android builds
docker run -v $(pwd):/app/ -it kuruk/dcl-godot-android-builder-rust

# Inside container
cargo run -- install --targets android linux
cargo run -- build --target android
cargo run -- export --target android --format apk --release
```

## 🔧 Troubleshooting

1. **Check system health**:
   ```bash
   cargo run -- doctor
   ```

2. **Windows long path issues**: Clone to a short path like `C:/gexplorer`

3. **Missing dependencies**: The doctor command will show what's missing and how to install it

4. **Android build failures**: Ensure NDK 28.1.13356709 is installed and ANDROID_SDK is set

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Code style and formatting
- Pull request process
- Testing requirements
- Documentation standards

## 📚 Documentation

- [Project Overview by Claude](https://github.com/decentraland/godot-explorer/blob/main/CLAUDE.md)

## 🔗 Links

- [Decentraland](https://decentraland.org)
- [Godot Engine Fork](https://github.com/decentraland/godotengine)
- [Discord Community](https://dcl.gg/discord)

---

Powered by the Decentraland DAO

![Decentraland DAO logo](https://bafkreibci6gg3wbjvxzlqpuh353upzrssalqqoddb6c4rez33bcagqsc2a.ipfs.nftstorage.link/)

## Local web editor preview

The `web` export uses `godot/web/preview.html` and the parent workspace’s sibling
`godotengine/bin/godot.web.template_debug.wasm32.nothreads.dlink.zip`. Build and export
from the parent workspace with `./build.sh engine wasm preview`. The template requires
`threads=no webgpu=yes opengl3=no dlink_enabled=yes`. Generated `build/` files stay out of Git.
Package `build/web/` through the editor’s mobile runtime package; the editor owns serving and
its scene relay. Its HTML response needs `Document-Isolation-Policy: isolate-and-credentialless`
for embedded WebGPU.

`--local-editor-preview --fake-deeplink decentraland://open?preview=<realm>&position=0,0`
starts an ephemeral default avatar directly in the scene, without the lobby, telemetry, account
panels, notification/connection polling, remote feature flags, or surrounding floating islands.
Scene content, materials, player input, mobile controls, and scene UI retain their normal paths.
The preview draws the default grass floor under the scene's parcels, using one shared
mesh with two triangles per parcel. It adds no surrounding terrain or grass blades.
Scene JavaScript runs in the web worker;
its system API bridge remains incomplete (for example, renderer RPC calls).
Normal mobile runs do not use this mode.

The web export disables the engine’s disk shader cache; browser pipeline caching still applies.
Local previews keep settings and content in memory for the current run, with a 128 MiB
content-cache eviction budget instead of the native 2 GB default. They never restore an old
IndexedDB filesystem into RAM at startup. Reloading a preview refetches its assets through
the browser HTTP cache. Adaptive graphics is disabled for predictable preview frame pacing.
The web pack excludes native
iOS/Android artifacts. Joystick textures import at 512 pixels instead of 1920. Hidden local
preview tabs pause scene processing and rendering after startup finishes, resuming when brought
to the foreground. Startup stays active when hidden so loading can finish.
Minimizing the embedded preview also pauses it; restoring keeps the same engine and player camera.
The paused instance retains memory until closed, and captures require restoring it first.
The HTML shell caps the render budget at `?maxpixels=` (the editor sends 1600×720), accepts
scoped screenshot/visibility/stop messages, and forwards scene builds as hot-reload updates.

Floor regression: open the editor's built-in **Blank** scene and **Preview → As a phone**.
The avatar should stand on grass at ground level without adding a floor asset. Change the
parcel layout and rebuild: grass should cover the new parcels without surrounding islands.
Before the fix, the same scene shows sky beneath the avatar.

Verify in Brave: open **Preview → As a phone**, check textured models and touch controls, build
a scene edit and confirm it appears, then move the preview to a tab and back. Only one preview
should run. **Minimize** leaves the editor usable; **Restore preview** resumes without reloading,
and **Close preview** releases it. Test the message bridge with `node --test godot/web/preview.test.mjs`. The assistant can request `view({target: 'mobile'})`
in either location. Compare browser/Wasm memory on the same scene and canvas dimensions.

For this browser fork, pull requests run formatting and native Rust check/clippy.
The inherited Android, native release and container-publishing jobs run only in the
upstream repository. The `test-godot-web` parent builds the pinned web engine/extension
and runs browser regressions; it needs read-only access to both private source submodules.
Its offline browser-dependency check covers the actual Wasm runtime crates. The lockfile
includes patched anyhow, bytes, keccak, protobuf, rand, Tokio and tracing-subscriber;
native-only dependencies and development tools require their own advisory review.
