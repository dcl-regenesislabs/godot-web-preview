# Scene UI input regression

Import this folder into the web editor, then open **Preview → As a phone**.
Alternatively, run `npm install` and `npm start` here and open the scene's preview
URL in the mobile client. This scene uses local UI callbacks and needs no scene server.

1. Close any previous mobile preview and open a fresh instance. Wait for the
   blue button to show `down 0 / up 0`. Do not click the world first.
2. Click the blue scene button once. It must show `down 1 / up 1`.
3. Click again, press Escape, then click again. It must show `down 3 / up 3`,
   with the cursor still visible.
4. Drag the world beside the button. The red box must move across the view as
   the camera rotates; button counts must stay unchanged. Click the button again.
5. Repeat in a detached preview and after minimizing/restoring the embedded view.

Before the timestamp fix, the first click showed `down 0 / up 1`: the engine
assigned timestamp zero to the first press, and the SDK ignored it as old input.
Buttons using `onMouseDown` consequently ignored that press.
