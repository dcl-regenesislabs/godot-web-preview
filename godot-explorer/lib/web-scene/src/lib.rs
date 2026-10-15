//! Scene Web Workers for the web build: spawn, exchange CRDT frames, terminate.
//! Events arrive on the browser main thread, between Godot frames.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Crdt,
    Log,
    Error,
    Exit,
    /// A JSON `{id, op, args}` API call; answer it with [`reply`].
    Call,
    Comms,
    CommsAdapter,
    CommsState,
}

/// Called for every message a scene Worker sends; the payload was already copied out.
pub type EventHandler = fn(scene_id: i32, kind: EventKind, payload: Vec<u8>);

#[cfg(target_os = "emscripten")]
mod imp {
    use std::cell::Cell;

    use super::*;

    type EventCb = extern "C" fn(i32, i32, i32);

    extern "C" {
        fn dcl_scene_spawn(
            scene_id: i32,
            worker_src: *const u8,
            worker_src_len: i32,
            init_json: *const u8,
            init_len: i32,
            cb: EventCb,
        );
        fn dcl_scene_take(scene_id: i32, dst: *mut u8);
        fn dcl_scene_post(scene_id: i32, kind: i32, data: *const u8, len: i32);
        fn dcl_scene_kill(scene_id: i32);
        fn dcl_scene_comms_send(
            scene_id: i32,
            data: *const u8,
            len: i32,
            recipient: *const u8,
            recipient_len: i32,
        ) -> i32;
    }

    thread_local! {
        static HANDLER: Cell<Option<EventHandler>> = const { Cell::new(None) };
    }

    extern "C" fn on_event(scene_id: i32, kind: i32, len: i32) {
        let mut payload = vec![0u8; len as usize];
        unsafe { dcl_scene_take(scene_id, payload.as_mut_ptr()) };
        let kind = match kind {
            1 => EventKind::Crdt,
            2 => EventKind::Log,
            3 => EventKind::Error,
            5 => EventKind::Call,
            6 => EventKind::Comms,
            7 => EventKind::CommsAdapter,
            8 => EventKind::CommsState,
            _ => EventKind::Exit,
        };
        if let Some(handler) = HANDLER.with(|h| h.get()) {
            handler(scene_id, kind, payload);
        }
    }

    pub fn spawn(scene_id: i32, worker_src: &str, init_json: &str, handler: EventHandler) {
        HANDLER.with(|h| h.set(Some(handler)));
        unsafe {
            dcl_scene_spawn(
                scene_id,
                worker_src.as_ptr(),
                worker_src.len() as i32,
                init_json.as_ptr(),
                init_json.len() as i32,
                on_event,
            );
        }
    }

    pub fn post(scene_id: i32, frame: &[u8]) {
        unsafe { dcl_scene_post(scene_id, 1, frame.as_ptr(), frame.len() as i32) };
    }

    pub fn reply(scene_id: i32, json: &str) {
        unsafe { dcl_scene_post(scene_id, 2, json.as_ptr(), json.len() as i32) };
    }

    pub fn scene_state(scene_id: i32, json: &str) {
        unsafe { dcl_scene_post(scene_id, 5, json.as_ptr(), json.len() as i32) };
    }

    pub fn kill(scene_id: i32) {
        unsafe { dcl_scene_kill(scene_id) };
    }

    pub fn comms_receive(scene_id: i32, data: &[u8]) {
        unsafe { dcl_scene_post(scene_id, 3, data.as_ptr(), data.len() as i32) };
    }

    pub fn comms_adapter(scene_id: i32, json: &str) {
        unsafe { dcl_scene_post(scene_id, 4, json.as_ptr(), json.len() as i32) };
    }

    pub fn comms_send(scene_id: i32, data: &[u8], recipient: &str) -> bool {
        unsafe {
            dcl_scene_comms_send(
                scene_id,
                data.as_ptr(),
                data.len() as i32,
                recipient.as_ptr(),
                recipient.len() as i32,
            ) != 0
        }
    }
}

#[cfg(not(target_os = "emscripten"))]
mod imp {
    use super::*;

    pub fn spawn(_scene_id: i32, _worker_src: &str, _init_json: &str, _handler: EventHandler) {}
    pub fn post(_scene_id: i32, _frame: &[u8]) {}
    pub fn reply(_scene_id: i32, _json: &str) {}
    pub fn scene_state(_scene_id: i32, _json: &str) {}
    pub fn kill(_scene_id: i32) {}
    pub fn comms_receive(_scene_id: i32, _data: &[u8]) {}
    pub fn comms_adapter(_scene_id: i32, _json: &str) {}
    pub fn comms_send(_scene_id: i32, _data: &[u8], _recipient: &str) -> bool {
        false
    }
}

pub use imp::{comms_adapter, comms_receive, comms_send, kill, post, reply, scene_state, spawn};
