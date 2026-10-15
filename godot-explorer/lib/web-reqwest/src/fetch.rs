use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{Slot, SlotRef};

pub(crate) struct Request {
    pub method: String,
    pub url: String,
    pub headers_json: String,
    pub body: Vec<u8>,
    pub timeout_ms: i32,
    pub follow_redirects: bool,
}

/// `status` is the HTTP status, 0 for a network/CORS error and -1 for a timeout.
/// `meta` is `[final_url, [[name, value], ...]]` as JSON on success, the error message otherwise.
pub(crate) struct RawResponse {
    pub status: i32,
    pub body: Vec<u8>,
    pub meta: String,
}

pub(crate) struct FetchFuture {
    slot: SlotRef,
}

impl Future for FetchFuture {
    type Output = RawResponse;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<RawResponse> {
        let mut slot = self.slot.lock().unwrap();
        match slot.done.take() {
            Some(r) => Poll::Ready(r),
            None => {
                slot.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

#[cfg(target_os = "emscripten")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    type DoneCb = extern "C" fn(i32, i32, i32, i32);

    extern "C" {
        fn dcl_fetch_start(
            id: i32,
            method: *const u8,
            method_len: i32,
            url: *const u8,
            url_len: i32,
            headers_json: *const u8,
            headers_len: i32,
            body: *const u8,
            body_len: i32,
            timeout_ms: i32,
            follow_redirects: i32,
            done: DoneCb,
        );
        fn dcl_fetch_take(id: i32, body_dst: *mut u8, meta_dst: *mut u8);
    }

    thread_local! {
        static PENDING: RefCell<HashMap<i32, SlotRef>> = RefCell::new(HashMap::new());
        static NEXT_ID: Cell<i32> = const { Cell::new(1) };
    }

    extern "C" fn on_done(id: i32, status: i32, body_len: i32, meta_len: i32) {
        let mut body = vec![0u8; body_len as usize];
        let mut meta = vec![0u8; meta_len as usize];
        unsafe { dcl_fetch_take(id, body.as_mut_ptr(), meta.as_mut_ptr()) };
        let Some(slot) = PENDING.with(|p| p.borrow_mut().remove(&id)) else {
            return; // the future was dropped before completion
        };
        let waker = {
            let mut slot = slot.lock().unwrap();
            slot.done = Some(RawResponse {
                status,
                body,
                meta: String::from_utf8_lossy(&meta).into_owned(),
            });
            slot.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    pub(crate) fn fetch(req: Request) -> FetchFuture {
        let id = NEXT_ID.with(|n| {
            let id = n.get();
            n.set(id.wrapping_add(1).max(1));
            id
        });
        let slot: SlotRef = Arc::new(Mutex::new(Slot::default()));
        PENDING.with(|p| p.borrow_mut().insert(id, slot.clone()));
        unsafe {
            dcl_fetch_start(
                id,
                req.method.as_ptr(),
                req.method.len() as i32,
                req.url.as_ptr(),
                req.url.len() as i32,
                req.headers_json.as_ptr(),
                req.headers_json.len() as i32,
                req.body.as_ptr(),
                req.body.len() as i32,
                req.timeout_ms,
                req.follow_redirects as i32,
                on_done,
            );
        }
        FetchFuture { slot }
    }
}

#[cfg(not(target_os = "emscripten"))]
mod imp {
    use super::*;

    pub(crate) fn fetch(req: Request) -> FetchFuture {
        let slot = SlotRef::default();
        slot.lock().unwrap().done = Some(RawResponse {
            status: 0,
            body: vec![],
            meta: format!(
                "dcl-web-reqwest only works on wasm32-unknown-emscripten: {}",
                req.url
            ),
        });
        FetchFuture { slot }
    }
}

pub(crate) fn fetch(req: Request) -> FetchFuture {
    imp::fetch(req)
}
