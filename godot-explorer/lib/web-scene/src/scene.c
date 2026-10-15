// Scene Worker bridge. Worker messages are queued per scene on the JS side and pulled by Rust
// with dcl_scene_take after the event callback reports their size.
#include <emscripten.h>
#include <stdint.h>

// kind: 1 crdt from the scene, 2 log line, 3 error log line, 4 the scene stopped,
// 5 a JSON API call ({id, op, args}) the host answers with dcl_scene_post kind 2
typedef void (*dcl_scene_event_cb)(int scene_id, int kind, int len);

EM_JS(void, dcl_scene_spawn, (int scene_id, const char *worker_src, int worker_src_len, const char *init_json, int init_len, dcl_scene_event_cb cb), {
    const dec = new TextDecoder();
    const enc = new TextEncoder();
    const scenes = (Module.__dclScenes = Module.__dclScenes || new Map());
    const src = dec.decode(HEAPU8.slice(worker_src, worker_src + worker_src_len));
    const init = JSON.parse(dec.decode(HEAPU8.slice(init_json, init_json + init_len)));
    const url = URL.createObjectURL(new Blob([src], { type: 'text/javascript' }));
    const worker = new Worker(url, { name: 'scene-' + scene_id });
    URL.revokeObjectURL(url);
    const entry = { worker, pending: [] };
    scenes.set(scene_id, entry);
    const emit = (kind, bytes) => {
        if (scenes.get(scene_id) !== entry) return;
        entry.pending.push(bytes);
        wasmTable.get(cb)(scene_id, kind, bytes.length);
    };
    if (init.sceneComms) {
        entry.comms = new DclSceneComms(() => new Promise((resolve, reject) => {
            entry.adapterReply = { resolve, reject };
            emit(7, new Uint8Array(0));
        }), (sender, packet) => {
            const identity = enc.encode(sender);
            const bytes = new Uint8Array(1 + identity.length + packet.length);
            bytes[0] = identity.length;
            bytes.set(identity, 1);
            bytes.set(packet, 1 + identity.length);
            emit(6, bytes);
        }, (ready) => emit(8, new Uint8Array([ready ? 1 : 0])));
    }
    worker.onmessage = (ev) => {
        const m = ev.data;
        if (m.t === 'crdt') emit(1, new Uint8Array(m.data));
        else if (m.t === 'log') emit(m.error ? 3 : 2, enc.encode(m.msg));
        else if (m.t === 'exit') emit(4, new Uint8Array(0));
        else if (m.t === 'call') emit(5, enc.encode(m.json));
    };
    worker.onerror = (ev) => emit(3, enc.encode('[web-scene] worker error: ' + ev.message));
    init.t = 'init';
    worker.postMessage(init);
});

EM_JS(void, dcl_scene_take, (int scene_id, uint8_t *dst), {
    const entry = Module.__dclScenes && Module.__dclScenes.get(scene_id);
    const bytes = entry ? entry.pending.shift() : null;
    if (bytes && dst) HEAPU8.set(bytes, dst);
});

// kind: 1 a crdt frame, 2 the JSON reply to a call
EM_JS(void, dcl_scene_post, (int scene_id, int kind, const uint8_t *data, int len), {
    const entry = Module.__dclScenes && Module.__dclScenes.get(scene_id);
    if (!entry) return;
    const copy = HEAPU8.slice(data, data + len);
    if (kind === 2) entry.worker.postMessage({ t: 'reply', json: new TextDecoder().decode(copy) });
    else if (kind === 3) entry.worker.postMessage({ t: 'comms', data: copy.buffer }, [copy.buffer]);
    else if (kind === 4) {
        const result = JSON.parse(new TextDecoder().decode(copy));
        const pending = entry.adapterReply;
        entry.adapterReply = null;
        if (result.error) pending && pending.reject(new Error(result.error));
        else pending && pending.resolve(result.adapter);
    }
    else if (kind === 5) entry.worker.postMessage({ t: 'scene-state', json: new TextDecoder().decode(copy) });
    else entry.worker.postMessage({ t: 'crdt', data: copy.buffer }, [copy.buffer]);
});

EM_JS(void, dcl_scene_kill, (int scene_id), {
    const entry = Module.__dclScenes && Module.__dclScenes.get(scene_id);
    if (!entry) return;
    entry.comms && entry.comms.close();
    if (entry.adapterReply) entry.adapterReply.reject(new Error('Scene closed'));
    entry.worker.terminate();
    Module.__dclScenes.delete(scene_id);
});

EM_JS(int, dcl_scene_comms_send, (int scene_id, const uint8_t *data, int len, const char *recipient, int recipient_len), {
    const entry = Module.__dclScenes && Module.__dclScenes.get(scene_id);
    if (!entry || !entry.comms) return 0;
    const target = new TextDecoder().decode(HEAPU8.subarray(recipient, recipient + recipient_len));
    return entry.comms.send(HEAPU8.slice(data, data + len), target) ? 1 : 0;
});
