// Scene runtime for the web build: one dedicated Worker per scene.
// Provides the `Deno.core` surface that dcl/js/js_modules expect, so the same main.js loader and
// ~system/* modules run unchanged. CRDT frames travel as postMessage to the main thread, where
// dclgodot (wasm) applies them exactly like the deno ops in dcl/js/engine.rs.

const native = {
    fetch: self.fetch.bind(self),
    Headers: self.Headers,
    Response: self.Response,
    WebSocket: self.WebSocket,
    TextEncoder: self.TextEncoder,
    TextDecoder: self.TextDecoder,
};

let modules = {};
let sceneInfo = {};
let realm = {};
let worldTime = 54000;
let locale = 'en';
const subscriptions = new Set();
let sceneEvents = [];
let pendingFrame = null;
let recvWaiter = null;
let sentToRenderer = false;
let commsMessages = [];
let commsBytes = 0;
let commsEvents = [];
let commsEventBytes = 0;
let subscribedComms = false;
const unimplemented = new Set();
// API calls answered by the host (web/scene_host.rs on_call), by id
const calls = new Map();
let nextCallId = 1;

const post = (message, transfer) => self.postMessage(message, transfer || []);
const log = (msg, error) => post({ t: 'log', msg: String(msg), error: !!error });
const call = (op, args) => new Promise((resolve, reject) => {
    const id = nextCallId++;
    calls.set(id, { resolve, reject });
    post({ t: 'call', json: JSON.stringify({ id, op, args }) });
});
const action = value => call('scene_action', [value]);
const target = (x, y, z) => [x, y, z].some(Number.isNaN) ? null : [x, y, z];

const opsImpl = {
    op_require(name) {
        const source = modules[name];
        if (source === undefined) throw new Error(`module not found: ${name}`);
        return source;
    },
    op_log: (msg) => log(msg, false),
    op_error: (msg) => log(msg, true),
    op_crdt_send_to_renderer(buffer) {
        sentToRenderer = true;
        post({ t: 'crdt', data: buffer }, [buffer]);
    },
    op_crdt_recv_wait() {
        if (pendingFrame) return Promise.resolve(pendingFrame.byteLength);
        return new Promise((resolve) => { recvWaiter = resolve; });
    },
    op_crdt_recv_drain(out) {
        const frame = pendingFrame;
        if (!frame || frame.byteLength > out.byteLength) return 0;
        out.set(frame);
        pendingFrame = null;
        return frame.byteLength;
    },
    op_scene_debug_enabled: () => false,
    op_get_realm: () => realm,
    op_get_scene_information: async () => sceneInfo,
    op_get_world_time: () => worldTime,
    op_get_explorer_locale: () => locale,
    op_set_handled_promise_rejection_handler() {},
    op_promise_reject() {},
    op_subscribe(event) {
        if (subscriptions.has(event)) return;
        subscriptions.add(event);
        if (event === 'comms') subscribedComms = true;
        else void call('subscribe', [event]).catch(error => log(error.message, true));
    },
    op_unsubscribe(event) {
        subscriptions.delete(event);
        sceneEvents = sceneEvents.filter(e => e.generic.eventId !== event);
        if (event === 'comms') {
            subscribedComms = false;
            commsEvents = [];
            commsEventBytes = 0;
        } else void call('unsubscribe', [event]).catch(error => log(error.message, true));
    },
    op_send_batch: () => {
        const events = [...sceneEvents, ...commsEvents];
        sceneEvents = [];
        commsEvents = [];
        commsEventBytes = 0;
        return events;
    },
    op_get_connected_players: () => call('get_connected_players', []),
    op_get_players_in_scene: () => call('get_players_in_scene', []),
    op_get_player_data: (userId) => call('get_player_data', [userId]),
    op_move_player_to: (x, y, z, cx, cy, cz, ax, ay, az) => action({ kind: 'move', position: [x, y, z], camera: target(cx, cy, cz), avatar: target(ax, ay, az) }),
    op_trigger_emote: (id, mask) => action({ kind: 'emote', id, mask }),
    op_trigger_scene_emote: (src, looping, mask) => action({ kind: 'scene_emote', src, looping, mask }),
    op_stop_emote: () => action({ kind: 'stop_emote' }),
    op_teleport_to: (x, y, hasCoordinates, realm) => action({ kind: 'teleport', coordinates: hasCoordinates ? [x, y] : null, realm: realm || null }),
    op_change_realm: (realm, message) => action({ kind: 'change_realm', realm, message: message ?? null }),
    op_signed_fetch_headers: (url, method) => call('signed_fetch_headers', [url, method ?? null]),
    op_comms_send_binary_single: (data, recipient) => call('comms_send', [[2, ...data], recipient ?? null]),
    op_comms_send_string: (message) => call('comms_send', [[1, ...new native.TextEncoder().encode(message)], null]),
    op_comms_recv_binary: async () => {
        const messages = commsMessages;
        commsMessages = [];
        commsBytes = 0;
        return messages;
    },
};

const ops = new Proxy(opsImpl, {
    get(target, key) {
        if (key in target) return target[key];
        if (!unimplemented.has(key)) {
            unimplemented.add(key);
            log(`[web-scene] unimplemented op ${String(key)}`, true);
        }
        return () => { throw new Error(`Scene API ${String(key)} is unavailable in the browser preview`); };
    },
});

self.Deno = {
    core: {
        ops,
        evalContext(source, name) {
            try {
                return [(0, eval)(`${source}\n//# sourceURL=${name}`), null];
            } catch (e) {
                return [undefined, { thrown: e }];
            }
        },
        setUnhandledPromiseRejectionHandler() {},
        setHandledPromiseRejectionHandler() {},
    },
};

async function run(init) {
    modules = init.modules;
    sceneInfo = init.sceneInfo;
    realm = init.realm;
    worldTime = init.worldTime ?? worldTime;
    locale = init.locale ?? locale;

    (0, eval)(`${init.loader}\n//# sourceURL=dcl-loader.js`);
    // The loader installs deno-op based polyfills; a Worker has the real thing.
    Object.assign(self, native);

    const scene = (0, eval)(`${init.sceneCode}\n//# sourceURL=scene-main.js`);
    if (typeof scene.onStart === 'function') await scene.onStart();
    // Like dcl/js/mod.rs: onUpdate errors are logged (the first 10) and the scene keeps running,
    // unless it hits 10 without ever having sent anything to the renderer.
    let last = performance.now();
    let errors = 0;
    for (;;) {
        const now = performance.now();
        try {
            await scene.onUpdate((now - last) / 1000);
        } catch (e) {
            errors += 1;
            if (errors <= 10) log(`[web-scene] script error onUpdate: ${e && e.stack ? e.stack : e}`, true);
            if (errors === 10 && !sentToRenderer) throw new Error('too many errors without renderer interaction');
        }
        last = now;
    }
}

self.onmessage = (event) => {
    const msg = event.data;
    if (msg.t === 'init') {
        run(msg).catch((e) => {
            log(`[web-scene] scene stopped: ${e && e.stack ? e.stack : e}`, true);
            post({ t: 'exit' });
        });
    } else if (msg.t === 'reply') {
        const { id, ok, err } = JSON.parse(msg.json);
        const pending = calls.get(id);
        calls.delete(id);
        if (pending && err !== undefined) pending.reject(new Error(err));
        else if (pending) pending.resolve(ok);
    } else if (msg.t === 'scene-state') {
        const state = JSON.parse(msg.json);
        worldTime = state.worldTime;
        locale = state.locale;
        for (const event of state.events) {
            if (subscriptions.has(event.generic.eventId) && sceneEvents.length < 512) sceneEvents.push(event);
        }
    } else if (msg.t === 'comms') {
        const frame = new Uint8Array(msg.data);
        const data = frame.subarray(1);
        if (frame[0] === 2 && commsMessages.length < 500 && commsBytes + data.byteLength <= 4 * 1024 * 1024) {
            commsMessages.push(data);
            commsBytes += data.byteLength;
        } else if (frame[0] === 1 && subscribedComms && commsEvents.length < 500 && commsEventBytes + data.byteLength <= 4 * 1024 * 1024) {
            const decoder = new native.TextDecoder();
            const sender = decoder.decode(data.subarray(1, 1 + data[0]));
            const message = decoder.decode(data.subarray(1 + data[0]));
            commsEvents.push({ generic: { eventId: 'comms', eventData: JSON.stringify({ sender, message }) } });
            commsEventBytes += data.byteLength;
        }
    } else if (msg.t === 'crdt') {
        pendingFrame = new Uint8Array(msg.data);
        if (recvWaiter) {
            const resolve = recvWaiter;
            recvWaiter = null;
            resolve(pendingFrame.byteLength);
        }
    }
};
