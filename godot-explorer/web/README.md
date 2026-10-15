# Web build (WebGPU)

The explorer running in the browser on WebGPU. Findings and status are in [REPORT.md](REPORT.md).

## Layout

Clone these side by side:

```
godot-explorer/   this repo
godotengine/      the engine fork with the WebGPU driver (kuruk-mm/dclgodotengine)
emsdk/            https://github.com/emscripten-core/emsdk
```

## Build

1. **Emscripten 4.0.11, patched** for Rust side modules. Emscripten needs Python 3.10 or newer. On macOS the
   system `python3` is 3.9, so point it at a newer one (e.g. `brew install python@3.13`) in every shell
   that uses emsdk:
   ```bash
   export EMSDK_PYTHON=$(which python3.13)
   cd emsdk && ./emsdk install 4.0.11 && ./emsdk activate 4.0.11
   cd upstream/emscripten && patch -p1 < ../../../godot-explorer/web/emsdk-side-module-exports.patch
   ```
2. **Engine web template** (single-threaded, WebGPU only):
   ```bash
   export EMSDK_PYTHON=$(which python3.13)   # macOS, see step 1
   cd godotengine && source ../emsdk/emsdk_env.sh
   scons platform=web target=template_debug dlink_enabled=yes webgpu=yes opengl3=no threads=no
   ```
   The `web` export preset reads `../../godotengine/bin/godot.web.template_debug.wasm32.nothreads.dlink.zip`.
3. **Explorer editor and native library** (the editor imports the project with it):
   ```bash
   cd godot-explorer && cargo run -- install && cargo run -- build --no-default-features
   ```
4. **`dclgodot` for wasm.** Needs nightly Rust, because the template has no C++ exceptions and std must be rebuilt with `panic=abort`:
   ```bash
   rustup toolchain install nightly-2026-09-15 --component rust-src
   cd godot-explorer/lib && source ../web/wasm-env.sh
   CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_STRIP=true CARGO_PROFILE_RELEASE_LTO=false \
     cargo +nightly-2026-09-15 build --release -Zbuild-std=std,panic_abort \
     --target wasm32-unknown-emscripten --no-default-features --features use_pulse
   ```
5. **Export** to `build/web`. Arguments after the profile are passed to the explorer:
   ```bash
   web/tools/export-web.sh release --skip-lobby --guest-profile --realm=https://realm-provider-ea.decentraland.org/main --location=0,0
   ```

## Run

- In your browser: `node web/tools/serve-web.mjs build/web 8060`, then open http://localhost:8060/ in Chrome.
- Headless, with console logs and a screenshot: `cd web/tools && npm install`, then from the repo root
  `node web/tools/run-web.mjs build/web 120 shot.png`. Options:
  - `--headed`;
  - `--size=1728x1000 --dpr=2`;
  - `"--actions=60:shot:a.png;90:click:640,360;100:eval:devicePixelRatio"`.
- URL options: `?maxpixels=N` caps the canvas at N device pixels (default 1920×1080); `?maxpixels=native` turns the cap off.
