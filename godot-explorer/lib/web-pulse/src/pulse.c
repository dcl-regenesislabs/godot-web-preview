#include <emscripten.h>
#include <stdint.h>

typedef int (*pulse_event_cb)(int id, int kind, uint32_t value);

EM_JS(void, dcl_pulse_open, (int id, const uint8_t *url, int len, pulse_event_cb cb), {
    const sockets = (Module.__dclPulse = Module.__dclPulse || new Map());
    const entry = { socket: null, pending: null };
    sockets.set(id, entry);
    entry.socket = new DclPulseTransport(new TextDecoder().decode(HEAPU8.slice(url, url + len)), (kind, payload) => {
        if (sockets.get(id) !== entry) return false;
        entry.pending = payload instanceof Uint8Array ? payload : null;
        const accepted = wasmTable.get(cb)(id, kind, entry.pending ? entry.pending.length : payload || 0);
        entry.pending = null;
        if (kind >= 3) sockets.delete(id);
        return !!accepted;
    });
});

EM_JS(void, dcl_pulse_take, (int id, uint8_t *data), {
    const entry = Module.__dclPulse && Module.__dclPulse.get(id);
    if (entry && entry.pending) HEAPU8.set(entry.pending, data);
});

EM_JS(int, dcl_pulse_send, (int id, int channel, const uint8_t *data, int len), {
    const entry = Module.__dclPulse && Module.__dclPulse.get(id);
    return entry && entry.socket.send(channel, HEAPU8.slice(data, data + len)) ? 1 : 0;
});

EM_JS(void, dcl_pulse_close, (int id), {
    const sockets = Module.__dclPulse;
    const entry = sockets && sockets.get(id);
    if (!entry) return;
    sockets.delete(id);
    entry.socket.close();
});
