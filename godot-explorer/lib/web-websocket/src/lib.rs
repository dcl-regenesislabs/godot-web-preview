//! Browser `WebSocket` as async Rust, for wasm32-unknown-emscripten without threads.
//! All callbacks arrive on the browser main thread, between Godot frames.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The browser refused or dropped the connection (no detail is exposed to scripts).
    Failed,
    Closed(u16),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Failed => f.write_str("websocket error"),
            Error::Closed(code) => write!(f, "websocket closed ({code})"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Default)]
struct State {
    open: bool,
    closed: Option<Error>,
    incoming: VecDeque<Message>,
    waker: Option<Waker>,
}

type StateRef = Arc<Mutex<State>>;

pub struct WebSocket {
    id: i32,
    state: StateRef,
}

impl WebSocket {
    /// Resolves once the socket is open, or with the error that closed it first.
    pub async fn connect(url: &str, protocol: Option<&str>) -> Result<WebSocket, Error> {
        let socket = imp::open(url, protocol.unwrap_or(""));
        WaitOpen {
            state: socket.state.clone(),
        }
        .await?;
        Ok(socket)
    }

    pub fn send(&self, message: Message) -> Result<(), Error> {
        if let Some(e) = self.state.lock().unwrap().closed.clone() {
            return Err(e);
        }
        imp::send(self.id, message)
    }

    /// `None` once the socket is closed and every received message was consumed.
    pub async fn receive(&self) -> Option<Result<Message, Error>> {
        Receive {
            state: self.state.clone(),
        }
        .await
    }

    pub fn close(&self) {
        imp::close(self.id);
    }
}

impl Drop for WebSocket {
    fn drop(&mut self) {
        imp::close(self.id);
    }
}

struct WaitOpen {
    state: StateRef,
}

impl Future for WaitOpen {
    type Output = Result<(), Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut s = self.state.lock().unwrap();
        if let Some(e) = s.closed.clone() {
            return Poll::Ready(Err(e));
        }
        if s.open {
            return Poll::Ready(Ok(()));
        }
        s.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

struct Receive {
    state: StateRef,
}

impl Future for Receive {
    type Output = Option<Result<Message, Error>>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut s = self.state.lock().unwrap();
        if let Some(m) = s.incoming.pop_front() {
            return Poll::Ready(Some(Ok(m)));
        }
        if s.closed.is_some() {
            return Poll::Ready(None);
        }
        s.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

#[cfg(target_os = "emscripten")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    use super::*;

    type EventCb = extern "C" fn(i32, i32, i32);

    extern "C" {
        fn dcl_ws_open(
            id: i32,
            url: *const u8,
            url_len: i32,
            protocol: *const u8,
            protocol_len: i32,
            cb: EventCb,
        );
        fn dcl_ws_take(id: i32, dst: *mut u8);
        fn dcl_ws_send(id: i32, binary: i32, data: *const u8, len: i32) -> i32;
        fn dcl_ws_close(id: i32);
    }

    thread_local! {
        static SOCKETS: RefCell<HashMap<i32, StateRef>> = RefCell::new(HashMap::new());
        static NEXT_ID: Cell<i32> = const { Cell::new(1) };
    }

    extern "C" fn on_event(id: i32, kind: i32, len: i32) {
        let payload_len = if kind == 1 || kind == 2 {
            len as usize
        } else {
            0
        };
        let mut payload = vec![0u8; payload_len];
        unsafe { dcl_ws_take(id, payload.as_mut_ptr()) };
        let Some(state) = SOCKETS.with(|s| s.borrow().get(&id).cloned()) else {
            return;
        };
        let waker = {
            let mut s = state.lock().unwrap();
            match kind {
                0 => s.open = true,
                1 => s.incoming.push_back(Message::Text(
                    String::from_utf8_lossy(&payload).into_owned(),
                )),
                2 => s.incoming.push_back(Message::Binary(payload)),
                3 => {
                    s.closed.get_or_insert(Error::Closed(len as u16));
                }
                _ => {
                    s.closed.get_or_insert(Error::Failed);
                }
            }
            s.waker.take()
        };
        if kind == 3 {
            SOCKETS.with(|s| s.borrow_mut().remove(&id));
        }
        if let Some(w) = waker {
            w.wake();
        }
    }

    pub(super) fn open(url: &str, protocol: &str) -> WebSocket {
        let id = NEXT_ID.with(|n| {
            let id = n.get();
            n.set(id.wrapping_add(1).max(1));
            id
        });
        let state = StateRef::default();
        SOCKETS.with(|s| s.borrow_mut().insert(id, state.clone()));
        unsafe {
            dcl_ws_open(
                id,
                url.as_ptr(),
                url.len() as i32,
                protocol.as_ptr(),
                protocol.len() as i32,
                on_event,
            );
        }
        WebSocket { id, state }
    }

    pub(super) fn send(id: i32, message: Message) -> Result<(), Error> {
        let (binary, bytes) = match &message {
            Message::Text(t) => (0, t.as_bytes()),
            Message::Binary(b) => (1, b.as_slice()),
        };
        match unsafe { dcl_ws_send(id, binary, bytes.as_ptr(), bytes.len() as i32) } {
            0 => Ok(()),
            _ => Err(Error::Failed),
        }
    }

    pub(super) fn close(id: i32) {
        unsafe { dcl_ws_close(id) };
    }
}

#[cfg(not(target_os = "emscripten"))]
mod imp {
    use super::*;

    pub(super) fn open(_url: &str, _protocol: &str) -> WebSocket {
        let state = StateRef::default();
        state.lock().unwrap().closed = Some(Error::Failed);
        WebSocket { id: 0, state }
    }
    pub(super) fn send(_id: i32, _message: Message) -> Result<(), Error> {
        Err(Error::Failed)
    }
    pub(super) fn close(_id: i32) {}
}
