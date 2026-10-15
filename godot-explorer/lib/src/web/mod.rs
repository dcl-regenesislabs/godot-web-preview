//! Browser (wasm32-unknown-emscripten, no threads) replacements for native-only APIs.
//! On native targets everything here re-exports the real implementation.

pub mod fs;
#[cfg(target_family = "wasm")]
pub mod rpc_socket;
#[cfg(any(target_family = "wasm", test))]
pub mod scene_actions;
#[cfg(target_family = "wasm")]
pub mod scene_comms;
#[cfg(target_family = "wasm")]
pub mod scene_host;

#[cfg(not(target_family = "wasm"))]
pub use tokio::task::spawn_blocking;

/// No blocking pool on the web: the closure runs right away on the main thread.
#[cfg(target_family = "wasm")]
pub fn spawn_blocking<F, R>(f: F) -> std::future::Ready<Result<R, BlockingTaskError>>
where
    F: FnOnce() -> R,
{
    std::future::ready(Ok(f()))
}

/// Mirrors `tokio::task::JoinError` for callers that format it; never constructed.
#[cfg(target_family = "wasm")]
#[derive(Debug)]
pub enum BlockingTaskError {}

#[cfg(target_family = "wasm")]
impl std::fmt::Display for BlockingTaskError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}
