# WebGPU texture round-trip regression

This scene imports a generated textured glTF, saves it to a `PackedScene`, and reloads it in the same frame. Both quads must retain the same green/brown checkerboard. Before the fix, the saved quad is magenta because serialization receives no image pixels from asynchronous GPU readback.

The scene also checks immediate pixel reads, mipmaps, RGB/RGBA/grayscale formats, source/readback mutation isolation, updates, texture replacement, and proxy reads.

From the parent workspace root, after installing the editor with `./build.sh native`:

```sh
./build.sh engine
./build.sh test webgpu-texture-roundtrip
node godot-explorer/web/tools/serve-web.mjs tests/webgpu-texture-roundtrip/build 8065
```

Open `http://localhost:8065/index.html` in Brave with WebGPU enabled. The scene and console must show `[TextureRoundtrip] PASS`; `window.textureRoundtripResult` is also available to browser automation. Both quads must match. A successful export alone does not run the test.

The fixture is a generated quad with an embedded 2×2 PNG; it has no external asset dependencies.
