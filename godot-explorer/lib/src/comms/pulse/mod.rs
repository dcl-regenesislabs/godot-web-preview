//! Pulse avatar-state transport, alongside LiveKit scene messages.
//!
//! Module map (protocol ported from bevy-explorer `crates/comms/src/pulse/` @ 3f65c164):
//! - [`transport`] — the byte-boundary seam (frames, status, disconnect codes, channel pair)
//! - `native` — the `pulse-enet` driver thread (rusty_enet, ENet-CSharp modified protocol)
//! - `wasm` — browser WebTransport via the Emscripten byte bridge
//! - [`decoder`] — quantized state → `rfc4::Movement` reconstruction + parcel grid
//! - [`pulse_room`] — connection state machine, handshake, `MessageProcessor` bridging
//! - [`lsd_realm`] — Local Scene Development preview → Pulse realm key (cross-repo contract)

pub mod decoder;
pub mod lsd_realm;
#[cfg(not(target_family = "wasm"))]
mod native;
pub mod pulse_room;
pub mod transport;
#[cfg(target_family = "wasm")]
mod wasm;

/// See `comms::consts::PULSE_ROOM_ID` (lives there, unconditional, so MessageProcessor's
/// transport-preference gate compiles without the `use_pulse` feature).
pub use crate::comms::consts::PULSE_ROOM_ID;
