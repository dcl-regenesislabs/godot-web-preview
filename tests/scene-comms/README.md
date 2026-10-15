# Scene server communication regression

Import this folder into the local web editor and open **Preview → As a phone**.
The editor must be running its browser scene server. The fixture declares
`authoritativeMultiplayer: true`; no deployed scene or persistent storage is used.

1. Wait for **Connected to scene server**, then click the blue panel.
2. Check that sent, replies, last sequence and synced server count all advance.
   The server logs `SCENE_COMMS_SERVER sequence=1`, with the mobile guest's sender
   address. The mobile scene logs a reply; the SDK accepts these events only from
   `authoritative-server` (the client callback has no sender context).
   The panel must also say **Server sees your player** and show its position.
   Walk toward the red cube, then click again: the server position changes and
   **near red cube** becomes true within two meters. The `PULSE_SERVER` log records
   the same result. The ping sends no coordinates; the server reads the player's
   engine-owned `Transform` and `PlayerIdentityData` received through Pulse.
3. Minimize and restore, then click again. Replies and synchronized state continue.
4. Move the preview to a tab. Its fresh client connects and can send another ping.
5. Edit and rebuild the scene. The old connection closes and the new scene can
   exchange messages. Close the preview and check its worker/connection disappear.

Before the transport fix, the mobile panel stays **Waiting for scene server** and
clicks never get a server reply. A server startup crash is also a failure: a local
click counter alone does not prove delivery.
Before the Pulse bridge, replies can work while **Server cannot see your player**
persists. To reproduce that state with the new build, add `--no-pulse` to the
mobile runtime's `args` query parameter and restart that preview instance.

The server increments the count and sends targeted replies through `registerMessages`.
It also replicates the count with `syncEntity`, so the fixture checks both messages
and authoritative CRDT state through the real scene server worker.

Transport queue, identity and lifecycle tests (no network required):

```sh
node --test godot-explorer/web/tools/scene-comms.test.mjs
node --test godot-explorer/web/tools/pulse.test.mjs
cd godot-explorer/lib
cargo test --locked --lib --no-default-features --features use_pulse comms::pulse:: -j 2
```
