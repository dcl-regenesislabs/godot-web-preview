# WebGPU buffer coherence regression

The fixture verifies synchronous vertex-buffer reads after creation, CPU updates, clears
and copies, without waiting for a frame. It then binds the buffer to a read-only compute
shader followed by a writable shader. Read-only use must retain synchronous readback; writable
use must eventually return the GPU result instead of the old CPU mirror.

From the parent workspace:

```sh
./build.sh test webgpu-buffer-coherence
node godot-explorer/web/tools/serve-web.mjs tests/webgpu-buffer-coherence/build 8065
```

Open `http://localhost:8065/`. The console must report `[BufferCoherence] PASS` and
`window.bufferCoherenceResult` must be `"PASS"`. Before the fix, CPU updates and GPU
writes both report stale data. The asynchronous GPU readback has a 180-frame deadline.
