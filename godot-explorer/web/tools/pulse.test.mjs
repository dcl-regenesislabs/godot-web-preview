import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import vm from 'node:vm'
import test from 'node:test'

const source = await readFile(new URL('../pulse.js', import.meta.url), 'utf8')
const tick = () => new Promise(resolve => setImmediate(resolve))
const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b }); return { promise, resolve, reject } }
const framed = bytes => { const out = new Uint8Array(4 + bytes.length); new DataView(out.buffer).setUint32(0, bytes.length); out.set(bytes, 4); return out }

function setup({ slowConnect = false, accept = () => true, url = 'https://pulse.example:7743/' } = {}) {
  const instances = [], events = [], timers = new Map(), listeners = new Map()
  class Transport {
    writes = []
    datagramWrites = []
    connection = deferred()
    closure = deferred()
    ready = this.connection.promise
    closed = this.closure.promise
    stream = {
      readable: new ReadableStream({ start: controller => { this.streamInput = controller } }),
      writable: { getWriter: () => ({ write: bytes => { this.writes.push(Array.from(bytes)); return this.writePending?.promise ?? Promise.resolve() }, abort: async () => { this.aborted = true } }) }
    }
    datagrams = {
      maxDatagramSize: 1200,
      readable: new ReadableStream({ start: controller => { this.datagramInput = controller } }),
      writable: { getWriter: () => ({ write: bytes => { this.datagramWrites.push(Array.from(bytes)); return this.datagramPending?.promise ?? Promise.resolve() }, abort: async () => {} }) }
    }
    constructor(url) { this.url = url; instances.push(this); if (!slowConnect) this.connection.resolve() }
    async createBidirectionalStream() { return this.stream }
    close(info) { this.closeInfo = info; this.closure.resolve(info) }
  }
  const context = vm.createContext({
    URL, queueMicrotask, WebTransport: Transport,
    addEventListener(name, callback) { listeners.set(name, callback) },
    removeEventListener(name) { listeners.delete(name) },
    setTimeout(fn) { const id = {}; timers.set(id, fn); return id },
    clearTimeout(id) { timers.delete(id) }
  })
  vm.runInContext(source, context)
  const socket = new context.DclPulseTransport(url, (kind, data) => { events.push({ kind, data }); return accept(kind, data) })
  return { socket, instances, events, timers, listeners }
}

test('frames reliable messages and movement datagrams like the Pulse server', async () => {
  const s = setup(); await tick()
  assert.deepEqual(s.events.map(e => e.kind), [0])
  assert.equal(s.socket.send(0, new Uint8Array([10, 20])), true)
  assert.equal(s.socket.send(1, new Uint8Array([30, 40])), true)
  await tick()
  assert.deepEqual(s.instances[0].writes, [[0, 0, 0, 2, 10, 20]])
  assert.deepEqual(s.instances[0].datagramWrites, [[1, 0, 0, 0, 1, 30, 40]])
  s.socket.close()
})

test('reassembles fragmented and coalesced stream frames and reads bare datagrams', async () => {
  const s = setup(); await tick()
  const stream = s.instances[0].streamInput
  for (const byte of framed(new Uint8Array([5, 6, 7]))) stream.enqueue(new Uint8Array([byte]))
  stream.enqueue(new Uint8Array([...framed(new Uint8Array([])), ...framed(new Uint8Array([8]))]))
  s.instances[0].datagramInput.enqueue(new Uint8Array([9, 10]))
  await tick()
  assert.deepEqual(s.events.filter(e => e.kind === 1).map(e => Array.from(e.data)), [[5, 6, 7], [], [8]])
  assert.deepEqual(Array.from(s.events.find(e => e.kind === 2).data), [9, 10])
  s.socket.close()
})

test('rejects oversized reliable frames before allocating their payload', async () => {
  const s = setup(); await tick()
  s.instances[0].streamInput.enqueue(new Uint8Array([0, 0, 16, 1]))
  await tick()
  assert.equal(s.socket.closed, true)
  assert.deepEqual(s.events.map(e => e.kind), [0, 3])
})

test('bounds reliable queues and retains only fresh movement under backpressure', async () => {
  const s = setup(); await tick()
  const transport = s.instances[0]
  transport.writePending = deferred()
  transport.datagramPending = deferred()
  for (let i = 0; i < 64; i++) assert.equal(s.socket.send(0, new Uint8Array(4096)), true)
  assert.equal(s.socket.send(0, new Uint8Array([1])), false)
  s.socket.send(1, new Uint8Array([1])); await tick()
  for (let i = 2; i <= 20; i++) s.socket.send(1, new Uint8Array([i]))
  await tick()
  assert.equal(transport.datagramWrites.length, 1)
  transport.datagramPending.resolve(); await tick()
  assert.deepEqual(transport.datagramWrites.map(bytes => bytes.at(-1)), [1, 20])
  s.socket.close()
  transport.writePending.resolve()
})

test('closes a full reliable receive queue instead of silently losing state', async () => {
  const s = setup({ accept: kind => kind !== 1 }); await tick()
  s.instances[0].streamInput.enqueue(framed(new Uint8Array([1])))
  await tick()
  assert.equal(s.socket.closed, true)
  assert.deepEqual(s.events.map(e => e.kind), [0, 1, 3])
})

test('preserves terminal server disconnect codes and reports closure once', async () => {
  const s = setup(); await tick()
  s.instances[0].closure.resolve({ closeCode: 5 })
  await tick()
  assert.deepEqual(s.events.map(e => [e.kind, e.kind === 3 ? e.data : 0]), [[0, 0], [3, 5]])
  assert.equal(s.socket.send(0, new Uint8Array([1])), false)
  assert.equal(s.timers.size, 0)
})

test('teardown during connect and connect timeout leave no active transport', async () => {
  const s = setup({ slowConnect: true }); await tick()
  s.socket.close()
  s.instances[0].connection.resolve(); await tick()
  assert.equal(s.events.length, 0)
  assert.equal(s.instances[0].closeInfo.closeCode, 1)
  const t = setup({ slowConnect: true }); await tick()
  Array.from(t.timers.values())[0]()
  assert.equal(t.socket.closed, true)
  assert.deepEqual(t.events.map(e => e.kind), [4])
  assert.equal(t.timers.size, 0)
})

test('page teardown closes Pulse and removes its listener', async () => {
  const s = setup(); await tick()
  s.listeners.get('pagehide')()
  await tick()
  assert.equal(s.socket.closed, true)
  assert.equal(s.listeners.size, 0)
  assert.deepEqual(s.events.map(e => e.kind), [0, 3])
})

test('a failed datagram is dropped while a failed reliable write closes the session', async () => {
  const s = setup(); await tick()
  const t = s.instances[0]
  t.datagramPending = deferred()
  s.socket.send(1, new Uint8Array([1])); await tick()
  t.datagramPending.reject(new Error('Dropped datagram')); await tick()
  assert.equal(s.socket.closed, false)
  t.writePending = deferred()
  s.socket.send(0, new Uint8Array([1])); await tick()
  t.writePending.reject(new Error('Broken stream')); await tick()
  assert.equal(s.socket.closed, true)
  assert.deepEqual(s.events.map(e => e.kind), [0, 3])
})

test('rejects insecure and credential-bearing endpoints before connecting', async () => {
  for (const url of ['http://pulse.example/', 'https://user:password@pulse.example/']) {
    const s = setup({ url }); await tick()
    assert.equal(s.instances.length, 0)
    assert.deepEqual(s.events.map(e => e.kind), [4])
  }
})
