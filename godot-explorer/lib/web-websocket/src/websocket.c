// Browser WebSocket bridge. Events are queued per socket on the JS side and pulled by Rust
// with dcl_ws_take after the event callback reports their size.
#include <emscripten.h>
#include <stdint.h>

// kind: 0 open, 1 text, 2 binary, 3 close (len = close code), 4 error
typedef void (*dcl_ws_event_cb)(int id, int kind, int len);

EM_JS(void, dcl_ws_open, (int id, const char *url, int url_len, const char *protocol, int protocol_len, dcl_ws_event_cb cb), {
    const dec = new TextDecoder();
    const sockets = (Module.__dclWs = Module.__dclWs || new Map());
    const u = dec.decode(HEAPU8.slice(url, url + url_len));
    const p = protocol_len > 0 ? dec.decode(HEAPU8.slice(protocol, protocol + protocol_len)) : undefined;
    const entry = { ws: null, pending: [] };
    sockets.set(id, entry);
    const emit = (kind, payload, len) => {
        entry.pending.push(payload);
        wasmTable.get(cb)(id, kind, len);
    };
    let ws;
    try {
        ws = p ? new WebSocket(u, p) : new WebSocket(u);
    } catch (e) {
        setTimeout(() => emit(4, null, 0), 0);
        return;
    }
    entry.ws = ws;
    ws.binaryType = 'arraybuffer';
    ws.onopen = () => emit(0, null, 0);
    ws.onmessage = (ev) => {
        if (typeof ev.data === 'string') {
            const bytes = new TextEncoder().encode(ev.data);
            emit(1, bytes, bytes.length);
        } else {
            const bytes = new Uint8Array(ev.data);
            emit(2, bytes, bytes.length);
        }
    };
    ws.onerror = () => emit(4, null, 0);
    ws.onclose = (ev) => { emit(3, null, ev.code); sockets.delete(id); };
});

EM_JS(void, dcl_ws_take, (int id, uint8_t *dst), {
    const entry = Module.__dclWs && Module.__dclWs.get(id);
    const payload = entry ? entry.pending.shift() : null;
    if (payload && dst) HEAPU8.set(payload, dst);
});

// Returns 0 on success, -1 when the socket is not open.
EM_JS(int, dcl_ws_send, (int id, int binary, const uint8_t *data, int len), {
    const entry = Module.__dclWs && Module.__dclWs.get(id);
    if (!entry || !entry.ws || entry.ws.readyState !== 1) return -1;
    const bytes = HEAPU8.slice(data, data + len);
    entry.ws.send(binary ? bytes : new TextDecoder().decode(bytes));
    return 0;
});

EM_JS(void, dcl_ws_close, (int id), {
    const entry = Module.__dclWs && Module.__dclWs.get(id);
    if (entry && entry.ws) entry.ws.close();
});
