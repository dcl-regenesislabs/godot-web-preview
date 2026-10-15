use std::sync::Arc;

use godot::prelude::*;
use tokio::runtime::{Handle, Runtime};

use crate::godot_classes::dcl_global::DclGlobal;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct TokioRuntime {
    pub runtime: Option<Arc<Runtime>>,
}

#[godot_api]
impl INode for TokioRuntime {
    fn init(_base: Base<Node>) -> Self {
        #[cfg(not(target_family = "wasm"))]
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("dcl-godot-tokio")
            .build();
        // No threads on the web: tasks run on the main thread, driven from `process`.
        #[cfg(target_family = "wasm")]
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        match rt {
            Ok(rt) => Self {
                runtime: Some(Arc::new(rt)),
            },
            Err(e) => {
                tracing::error!("{e}");
                Self { runtime: None }
            }
        }
    }

    /// Runs the tasks that became ready since the last frame (fetch/WebSocket callbacks,
    /// timers, channel wakeups). Yielding a few times lets freshly woken tasks chain.
    #[cfg(target_family = "wasm")]
    fn process(&mut self, _delta: f64) {
        if let Some(rt) = self.runtime.as_ref() {
            rt.block_on(async {
                for _ in 0..8 {
                    tokio::task::yield_now().await;
                }
            });
        }
    }
}

#[godot_api]
impl TokioRuntime {
    /// May return the handle to the tokio runtime, or `None` if no runtime handle is obtainable.
    pub fn try_get_handle(&self) -> Option<&Handle> {
        match self.runtime.as_ref() {
            Some(rt) => Some(rt.handle()),
            None => None,
        }
    }

    /// Panics if handle is not found!
    pub fn get_handle(&self) -> &Handle {
        self.try_get_handle().expect("Failed to get handle!")
    }
}

impl TokioRuntime {
    pub fn static_clone_handle() -> Option<Handle> {
        Some(
            DclGlobal::try_singleton()?
                .bind()
                .tokio_runtime
                .bind()
                .try_get_handle()?
                .clone(),
        )
    }

    pub fn spawn<F>(future: F)
    where
        F: futures_util::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        if let Some(handle) = Self::static_clone_handle() {
            handle.spawn(future);
        } else {
            #[cfg(target_family = "wasm")]
            tracing::error!("TokioRuntime::spawn before the runtime exists; task dropped");
            #[cfg(not(target_family = "wasm"))]
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Runtime::new();
                if runtime.is_err() {
                    panic!("Failed to create runtime {:?}", runtime.err());
                }
                let runtime = runtime.unwrap();

                runtime.block_on(async move {
                    future.await;
                });
            });
        }
    }
}
