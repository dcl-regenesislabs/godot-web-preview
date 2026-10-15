import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import vm from 'node:vm'
import test from 'node:test'

const source = await readFile(new URL('../scene-comms.js', import.meta.url), 'utf8')
const workerSource = await readFile(new URL('../../lib/src/web/scene_worker.js', import.meta.url), 'utf8')
const tick = () => new Promise(resolve => setImmediate(resolve))

function setup() {
  const rooms = [], timers = new Map(), sent = [], received = [], states = []
  class Room {
    handlers = new Map()
    localParticipant = { publishData: async (packet, options) => sent.push({ packet, options }) }
    constructor() { rooms.push(this) }
    on(event, callback) { this.handlers.set(event, callback) }
    async connect(url, token, options) { this.connection = { url, token, options } }
    async disconnect() { this.disconnected = true }
    removeAllListeners() { this.handlers.clear() }
    emit(event, ...args) { this.handlers.get(event)?.(...args) }
  }
  const context = vm.createContext({
    URL, queueMicrotask, console: { info() {}, warn() {} },
    setTimeout(callback, delay) { const id = {}; timers.set(id, { callback, delay }); return id },
    clearTimeout(id) { timers.delete(id) },
    LivekitClient: { Room, RoomEvent: { DataReceived: 'data', Reconnecting: 'reconnecting', Reconnected: 'reconnected', Disconnected: 'disconnected' } }
  })
  vm.runInContext(source, context)
  let resolveAdapter
  const comms = new context.DclSceneComms(
    () => new Promise(resolve => { resolveAdapter = resolve }),
    (sender, packet) => received.push({ sender, packet }),
    ready => states.push(ready)
  )
  return { comms, rooms, timers, sent, received, states, authorize: () => resolveAdapter('livekit:wss://example.test/room?access_token=fixture-token') }
}

test('buffers startup messages and publishes reliably to the authoritative server', async () => {
  const s = setup()
  assert.equal(s.comms.send(new Uint8Array([2, 8, 42]), 'authoritative-server'), true)
  assert.equal(s.sent.length, 0)
  s.authorize()
  await tick()
  assert.equal(s.sent.length, 1)
  assert.deepEqual(Array.from(s.sent[0].options.destinationIdentities), ['authoritative-server'])
  assert.equal(s.sent[0].options.reliable, true)
  assert.equal(s.rooms[0].connection.options.autoSubscribe, false)
  assert.equal(s.rooms[0].connection.url.includes('fixture-token'), false)
  assert.deepEqual(s.states, [true])
  s.comms.close()
})

test('preserves server identity and rejects invalid senders and oversized packets', async () => {
  const s = setup()
  s.authorize(); await tick()
  s.rooms[0].emit('data', new Uint8Array([2, 9]), { identity: 'authoritative-server' })
  s.rooms[0].emit('data', new Uint8Array([2, 9]), { identity: 'forged-server' })
  s.rooms[0].emit('data', new Uint8Array(65537), { identity: 'authoritative-server' })
  assert.equal(s.received.length, 1)
  assert.equal(s.received[0].sender, 'authoritative-server')
  assert.equal(s.comms.send(new Uint8Array([2]), 'forged-server'), false)
  s.comms.close()
})

test('bounds disconnected queues by message count and bytes', () => {
  const a = setup()
  for (let i = 0; i < 500; i++) assert.equal(a.comms.send(new Uint8Array([2]), ''), true)
  assert.equal(a.comms.send(new Uint8Array([2]), ''), false)
  const b = setup()
  for (let i = 0; i < 64; i++) assert.equal(b.comms.send(new Uint8Array(65536), ''), true)
  assert.equal(b.comms.send(new Uint8Array([2]), ''), false)
  a.comms.close(); b.comms.close()
})

test('reconnect updates readiness and scene teardown cancels retries', async () => {
  const s = setup()
  s.authorize(); await tick()
  s.rooms[0].emit('reconnecting')
  s.comms.send(new Uint8Array([2]), '')
  assert.equal(s.sent.length, 0)
  s.rooms[0].emit('reconnected'); await tick()
  assert.equal(s.sent.length, 1)
  assert.deepEqual(s.states, [true, false, true])
  s.rooms[0].emit('disconnected')
  assert.equal(s.timers.size, 1)
  s.comms.close()
  assert.equal(s.timers.size, 0)
  assert.equal(s.rooms[0].disconnected, true)
  assert.equal(s.comms.send(new Uint8Array([2]), ''), false)
})

test('closing during authorization cannot create an orphan room', async () => {
  const s = setup()
  s.comms.close(); s.authorize(); await tick()
  assert.equal(s.rooms.length, 0)
  assert.equal(s.timers.size, 0)
})

test('a replaced room cannot drain messages queued during reconnect', async () => {
  const s = setup()
  s.authorize(); await tick()
  let finishOldSend
  s.rooms[0].localParticipant.publishData = () => new Promise(resolve => { finishOldSend = resolve })
  s.comms.send(new Uint8Array([1]), '')
  await tick()
  s.rooms[0].emit('disconnected')
  s.comms.send(new Uint8Array([2]), '')
  Array.from(s.timers.values())[0].callback()
  s.authorize(); await tick()
  assert.equal(s.rooms.length, 2)
  finishOldSend(); await tick()
  assert.equal(s.sent.length, 1)
  assert.deepEqual(Array.from(s.sent[0].packet), [2])
  assert.equal(s.comms.queue.length, 0)
  s.comms.close()
})

test('scene worker forwards binary requests and drains bounded server replies once', async () => {
  const posted = []
  const context = vm.createContext({ fetch, Headers, Response, WebSocket, TextEncoder, TextDecoder, postMessage: value => posted.push(value) })
  context.self = context
  vm.runInContext(workerSource, context)
  const promise = context.Deno.core.ops.op_comms_send_binary_single(new Uint8Array([10, 20]), 'authoritative-server')
  const call = JSON.parse(posted[0].json)
  assert.deepEqual(call.args, [[2, 10, 20], 'authoritative-server'])
  context.onmessage({ data: { t: 'reply', json: JSON.stringify({ id: call.id, ok: null }) } })
  await promise
  const reply = new Uint8Array([2, 20, ...new TextEncoder().encode('authoritative-server'), 10, 20])
  for (let i = 0; i < 600; i++) context.onmessage({ data: { t: 'comms', data: reply.buffer } })
  const received = await context.Deno.core.ops.op_comms_recv_binary()
  assert.equal(received.length, 500)
  assert.deepEqual(Array.from(received[0]), Array.from(reply.subarray(1)))
  assert.equal((await context.Deno.core.ops.op_comms_recv_binary()).length, 0)
  reply[0] = 1
  context.Deno.core.ops.op_subscribe('comms')
  context.onmessage({ data: { t: 'comms', data: reply.buffer } })
  const events = context.Deno.core.ops.op_send_batch()
  assert.equal(events[0].generic.eventId, 'comms')
  assert.equal(JSON.parse(events[0].generic.eventData).sender, 'authoritative-server')
  assert.equal(context.Deno.core.ops.op_send_batch().length, 0)
})
