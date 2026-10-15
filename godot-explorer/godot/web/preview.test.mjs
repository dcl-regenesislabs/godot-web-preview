import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const html = readFileSync(new URL('./preview.html', import.meta.url), 'utf8')
const script = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)]
  .map((match) => match[1]).find((source) => source.includes('const config = $GODOT_CONFIG;'))

function setup(args = '--local-editor-preview') {
  const sent = []
  const visibility = []
  const listeners = new Map()
  const identity = { session: 'editor', instance: 'preview' }
  let channel
  const document = {
    hidden: false,
    addEventListener: (name, callback) => listeners.set(name, callback),
    getElementById: () => ({})
  }
  const window = {
    addEventListener() {},
    setLocalPreviewVisible: (value) => visibility.push(value)
  }
  vm.runInNewContext(script.replace('$GODOT_CONFIG', '{}') + '\nwindow.launchArgs = args;', {
    window, document, URLSearchParams,
    location: { search: `?args=${encodeURIComponent(args)}&previewSession=editor&previewInstance=preview` },
    navigator: {},
    console: { warn() {}, error() {} },
    BroadcastChannel: class {
      constructor() { channel = this }
      postMessage(message) { sent.push(message) }
    }
  })
  return {
    sent, visibility, window,
    receive: (message) => channel.onmessage({ data: { ...identity, ...message } }),
    hidden: (value) => { document.hidden = value; listeners.get('visibilitychange')() }
  }
}

test('local previews use mobile touch input instead of desktop pointer capture', () => {
  assert.ok(setup().window.launchArgs.includes('--force-mobile'))
  assert.equal(setup('--local-editor-preview --force-mobile').window.launchArgs.filter(arg => arg === '--force-mobile').length, 1)
  assert.ok(!setup('--skip-lobby').window.launchArgs.includes('--force-mobile'))
})

test('local previews enable Pulse and respect an explicit opt-out', () => {
  assert.ok(setup().window.launchArgs.includes('--pulse'))
  assert.ok(!setup('--local-editor-preview --no-pulse').window.launchArgs.includes('--pulse'))
  assert.equal(setup('--local-editor-preview --pulse').window.launchArgs.filter(arg => arg === '--pulse').length, 1)
  assert.ok(!setup('--skip-lobby').window.launchArgs.includes('--pulse'))
})

test('minimize survives startup and browser visibility cannot resume it', async () => {
  const preview = setup()
  await preview.receive({ type: 'visibility', visible: false })
  assert.deepEqual(preview.visibility, [])
  preview.window.localPreviewReady()
  assert.equal(preview.visibility.at(-1), false)
  preview.hidden(true)
  preview.hidden(false)
  assert.equal(preview.visibility.at(-1), false)
  await preview.receive({ type: 'visibility', visible: true })
  assert.equal(preview.visibility.at(-1), true)
  preview.hidden(true)
  await preview.receive({ type: 'visibility', visible: true })
  assert.equal(preview.visibility.at(-1), false)
  preview.hidden(false)
  assert.equal(preview.visibility.at(-1), true)
})

test('visibility messages are scoped to the active editor and preview', async () => {
  const preview = setup()
  preview.window.localPreviewReady()
  await preview.receive({ type: 'visibility', session: 'other', visible: false })
  await preview.receive({ type: 'visibility', instance: 'old', visible: false })
  await preview.receive({ type: 'visibility', visible: 'false' })
  assert.deepEqual(preview.visibility, [true])
})

test('a paused preview refuses a capture instead of returning a stale frame', async () => {
  const preview = setup()
  preview.window.localPreviewReady()
  await preview.receive({ type: 'visibility', visible: false })
  await preview.receive({ type: 'capture', requestId: 'picture' })
  assert.equal(preview.sent.at(-1).requestId, 'picture')
  assert.match(preview.sent.at(-1).error, /Restore/)
  assert.equal(preview.sent.at(-1).image, undefined)
})
