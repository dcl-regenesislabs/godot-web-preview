# Gameplay API regression

Import this folder into the local web editor and open **Preview → As a phone**.
The panel shows actual engine transforms and emote events, separate from the
request status. No server, deployment or account is needed.

1. Click **Move A**. Position must become `(4, 0, 4)`. The camera faces toward
   positive Z while the avatar faces positive X; their rotations differ.
2. Click **Move B**. Position becomes `(12, 0, 12)` and the camera turns toward
   negative X. Click **Outside (blocked)**: position stays inside the parcel.
3. Click **Wave**, then **Sit (loop)**. Watch the avatar animate and the emote
   text change. **Stop emote** returns the avatar to idle and reports the
   interrupted state. `assets/sit.glb` is the explorer's existing
   `godot/assets/no-export/emote_actions/SittingGround_v01.glb` fixture.
4. Click **Teleport prompt**, then cancel. Click **Realm prompt**, then cancel.
   Each must show the native confirmation dialog without moving the player
   or changing realm before confirmation.
5. **Ready** should become 1. Time should advance unless fixed skybox time was
   explicitly enabled; locale should match the client setting.
6. Minimize/restore and try movement again. Repeat after moving the preview to
   a tab and after rebuilding the scene. Close the preview when finished.

Before the fix, movement and emote requests report success but do nothing, and
navigation never shows a prompt. The runtime logs `unimplemented op` for each.

The fixture builds independently with `npm install && npm run build` in this
folder. Automated tests cover event subscriptions, player entry/exit, emote
starts, click results, action validation and worker replies:

```sh
node --test godot-explorer/web/tools/scene-gameplay.test.mjs
cd godot-explorer/lib
cargo test --locked --lib --no-default-features scene_ -j 2
```

Movement uses the native Godot handler, including its scene boundary checks.
Its optional interpolation duration is not implemented by that handler. Portable
experiences, file reads, wallet operations and external URL/NFT dialogs are outside
this test and remain unsupported in the browser runtime.
