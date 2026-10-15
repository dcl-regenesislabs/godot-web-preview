import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import vm from 'node:vm'
import test from 'node:test'

const source = await readFile(new URL('../../lib/src/web/scene_worker.js', import.meta.url), 'utf8')
const restricted = await readFile(new URL('../../lib/src/dcl/js/js_modules/RestrictedActions.js', import.meta.url), 'utf8')
const engine = await readFile(new URL('../../lib/src/dcl/js/js_modules/EngineApi.js', import.meta.url), 'utf8')

function setup() {
  const posted = []
  const context = vm.createContext({ fetch, Headers, Response, WebSocket, TextEncoder, TextDecoder, postMessage: value => posted.push(value) })
  context.self = context
  vm.runInContext(source, context)
  const load = code => vm.runInContext(`(function(){ const module = { exports: {} }; ${code}; return module.exports })()`, context)
  return {
    ops: context.Deno.core.ops, actions: load(restricted), engine: load(engine), posted,
    receive: data => context.onmessage({ data }),
    call: () => JSON.parse(posted.filter(message => message.t === 'call').at(-1).json)
  }
}

test('SDK movement preserves separate targets and waits for host acceptance', async () => {
  const s = setup()
  const promise = s.actions.movePlayerTo({ newRelativePosition: { x: 2, y: 3, z: 4 }, cameraTarget: { x: 5, y: 6, z: 7 } })
  assert.deepEqual(s.call().args, [{ kind: 'move', position: [2, 3, 4], camera: [5, 6, 7], avatar: null }])
  let done = false
  promise.then(() => { done = true }, () => {})
  await Promise.resolve()
  assert.equal(done, false)
  s.receive({ t: 'reply', json: JSON.stringify({ id: s.call().id, err: 'Too many scene actions in one frame' }) })
  await assert.rejects(promise, /Too many scene actions/)
})

test('emotes, stopping and navigation preserve SDK defaults and host errors', async () => {
  const s = setup()
  for (const [method, body, expected] of [
    ['triggerEmote', { predefinedEmote: 'wave' }, { kind: 'emote', id: 'wave', mask: -1 }],
    ['triggerSceneEmote', { src: 'dance.glb', loop: true, mask: 0 }, { kind: 'scene_emote', src: 'dance.glb', looping: true, mask: 0 }],
    ['stopEmote', {}, { kind: 'stop_emote' }],
    ['teleportTo', { realm: 'test.dcl.eth' }, { kind: 'teleport', coordinates: null, realm: 'test.dcl.eth' }],
    ['teleportTo', { worldCoordinates: { x: '2', y: '-3' } }, { kind: 'teleport', coordinates: [2, -3], realm: null }],
    ['changeRealm', { realm: 'test.dcl.eth', message: 'Continue the game' }, { kind: 'change_realm', realm: 'test.dcl.eth', message: 'Continue the game' }]
  ]) {
    const promise = s.actions[method](body)
    const call = s.call()
    assert.equal(call.op, 'scene_action')
    assert.deepEqual(call.args, [expected])
    s.receive({ t: 'reply', json: JSON.stringify({ id: call.id, ok: null }) })
    await promise
  }
  const denied = s.actions.teleportTo({ worldCoordinates: { x: '0', y: '0' } })
  s.receive({ t: 'reply', json: JSON.stringify({ id: s.call().id, err: 'Primary Player is outside the scene' }) })
  await assert.rejects(denied, /outside the scene/)
})

test('scene context supplies time, locale and bounded subscribed events exactly once', async () => {
  const s = setup()
  await s.engine.subscribe({ eventId: 'playerExpression' })
  const event = { generic: { eventId: 'playerExpression', eventData: '{"expressionId":"wave"}' } }
  s.receive({ t: 'scene-state', json: JSON.stringify({ worldTime: 1234, locale: 'es', events: Array(600).fill(event) }) })
  assert.equal(s.ops.op_get_world_time(), 1234)
  assert.equal(s.ops.op_get_explorer_locale(), 'es')
  assert.equal((await s.engine.sendBatch()).events.length, 512)
  assert.equal((await s.engine.sendBatch()).events.length, 0)
  s.receive({ t: 'scene-state', json: JSON.stringify({ worldTime: 1235, locale: 'es', events: [event] }) })
  await s.engine.unsubscribe({ eventId: 'playerExpression' })
  assert.equal(s.call().op, 'unsubscribe')
  assert.equal((await s.engine.sendBatch()).events.length, 0)
  s.receive({ t: 'scene-state', json: JSON.stringify({ worldTime: 1236, locale: 'es', events: [event] }) })
  assert.equal((await s.engine.sendBatch()).events.length, 0)
})

test('unsupported utilities fail explicitly instead of reporting success', async () => {
  const s = setup()
  await assert.rejects(s.actions.openExternalUrl({ url: 'https://example.org' }), /unavailable in the browser preview/)
  assert.equal(s.posted.some(message => message.t === 'call'), false)
})
