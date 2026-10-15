//! Pulse WebTransport byte bridge for the single-threaded Emscripten runtime.

pub enum Event {
    Connected,
    Message { bytes: Vec<u8>, reliable: bool },
    Disconnected(u32),
    Failed,
}

#[cfg(target_os = "emscripten")]
mod imp {
    use super::Event;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    type Handler = Box<dyn FnMut(Event) -> bool>;
    thread_local! {
        static NEXT: Cell<i32> = const { Cell::new(1) };
        static HANDLERS: RefCell<HashMap<i32, Handler>> = RefCell::new(HashMap::new());
    }

    extern "C" {
        fn dcl_pulse_open(
            id: i32,
            url: *const u8,
            len: i32,
            cb: extern "C" fn(i32, i32, u32) -> i32,
        );
        fn dcl_pulse_take(id: i32, data: *mut u8);
        fn dcl_pulse_send(id: i32, channel: i32, data: *const u8, len: i32) -> i32;
        fn dcl_pulse_close(id: i32);
    }

    extern "C" fn on_event(id: i32, kind: i32, value: u32) -> i32 {
        let event = match kind {
            0 => Event::Connected,
            1 | 2 => {
                if value > 4096 {
                    return 0;
                }
                let mut bytes = vec![0; value as usize];
                unsafe { dcl_pulse_take(id, bytes.as_mut_ptr()) };
                Event::Message {
                    bytes,
                    reliable: kind == 1,
                }
            }
            3 => Event::Disconnected(value),
            _ => Event::Failed,
        };
        HANDLERS.with(|handlers| {
            let mut handlers = handlers.borrow_mut();
            let accepted = handlers.get_mut(&id).is_some_and(|handler| handler(event));
            if kind >= 3 {
                handlers.remove(&id);
            }
            i32::from(accepted)
        })
    }

    pub fn open(url: &str, handler: impl FnMut(Event) -> bool + 'static) -> i32 {
        let id = NEXT.with(|next| {
            let id = next.get();
            next.set(id.wrapping_add(1).max(1));
            id
        });
        HANDLERS.with(|handlers| handlers.borrow_mut().insert(id, Box::new(handler)));
        unsafe { dcl_pulse_open(id, url.as_ptr(), url.len() as i32, on_event) };
        id
    }

    pub fn send(id: i32, channel: i32, bytes: &[u8]) -> bool {
        unsafe { dcl_pulse_send(id, channel, bytes.as_ptr(), bytes.len() as i32) != 0 }
    }

    pub fn close(id: i32) {
        HANDLERS.with(|handlers| handlers.borrow_mut().remove(&id));
        unsafe { dcl_pulse_close(id) };
    }
}

#[cfg(target_os = "emscripten")]
pub use imp::*;
