# Working on the browser preview

Read `CLAUDE.md` for build commands, [WebGPU porting lessons](docs/WEBGPU-PORTING.md)
before changing browser behavior, and `docs/UPSTREAMS.md` before updating sources.

- `godotengine/` and `godot-explorer/` are Git subtrees. Commit changes from this repository; do not create repositories inside those folders.
- Keep browser changes separate from upstream imports. Use `node scripts/upstream.mjs sync` on a feature branch; never replace an entire source folder with a fresh upstream checkout.
- Local builds use two jobs at background priority, as `build.sh` does. CI sets normal priority and the runner's core count. Each checkout owns its build directories.
- Keep root CI and the runtime README accurate when changing builds or packaging. Upstream workflows below the source folders do not run here.
- Preserve source licenses. Never commit generated binaries or read `.env*` files.
- Preserve the synchronous texture/buffer readback fixes, initial `main.crdt` flush and nonblocking browser frame pacing; run their linked regressions when changing those paths.
- Validate scene messages and Pulse player state separately. For mobile UI, inspect a runtime screenshot and test the first click, camera drag and minimize/restore together.
- Keep the porting guide current when a finding is superseded; the original spike report is historical evidence, not the current support matrix.
- Do not push, publish or create remote resources unless the user has authorized it.
