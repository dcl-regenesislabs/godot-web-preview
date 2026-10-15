# Browser frame pacing regression

A single-threaded web export must yield to the browser between Godot frames. This scene caps Godot at 5 FPS, warms up twelve engine frames, then independently counts engine and browser animation frames for five seconds. Before the fix, the native delay blocks the browser and both counts stay around 5 FPS. After the fix, browser frames stay above 30 FPS on a visible 60 Hz display while the engine stays below 8 FPS. Warmup excludes initial software shader compilation from the frame-limiter check.

From the parent workspace root, after installing the editor with `./build.sh native`:

```sh
./build.sh engine
./build.sh test webgpu-frame-pacing
node godot-explorer/web/tools/serve-web.mjs tests/webgpu-frame-pacing/build 8065
```

Open the page in a foreground browser tab. The console must print `[FramePacing] PASS`; automation can read `window.framePacingResult`. Repeat with an editor embedding the mobile preview: minimizing it must restore the editor's normal frame rate while the mobile scene stays paused.
