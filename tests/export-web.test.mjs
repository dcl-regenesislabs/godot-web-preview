import assert from 'node:assert/strict'
import { mkdtemp, mkdir, copyFile, writeFile, readFile, rm } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

async function fixture(t) {
  const root = await mkdtemp(path.join(tmpdir(), 'godot-export-'))
  t.after(() => rm(root, { recursive: true, force: true }))
  for (const dir of ['web/tools/node_modules/livekit-client/dist', 'bin', 'godot', '.bin/godot', 'build/web', 'lib/target/wasm32-unknown-emscripten/release']) await mkdir(path.join(root, dir), { recursive: true })
  const script = path.join(root, 'web/tools/export-web.sh')
  await copyFile(new URL('../godot-explorer/web/tools/export-web.sh', import.meta.url), script)
  await writeFile(path.join(root, 'lib/target/wasm32-unknown-emscripten/release/dclgodot.wasm'), 'fixture')
  await writeFile(path.join(root, '.bin/godot/godot4_bin'), `#!/bin/bash
if [ "\$EXPORT_FAIL" = 1 ]; then echo 'export failed'; exit 2; fi
if [ "\$EXPORT_SCRIPT_ERROR" = 1 ]; then echo 'SCRIPT ERROR: Parse Error: broken script'; fi
printf 'const %s = {"args":[],"canvasResizePolicy":2};\n' "\$EXPORT_CONFIG" > ../build/web/index.html
`, { mode: 0o755 })
  await writeFile(path.join(root, 'bin/npm'), '#!/bin/sh\nexit 0\n', { mode: 0o755 })
  for (const file of ['web/scene-comms.js', 'web/pulse.js', 'web/tools/node_modules/livekit-client/dist/livekit-client.umd.js', 'web/tools/node_modules/livekit-client/LICENSE']) await writeFile(path.join(root, file), 'fixture')
  return {
    root,
    run: (env, ...args) => spawnSync('bash', [script, 'release', ...args], { env: { ...process.env, PATH: path.join(root, 'bin') + path.delimiter + process.env.PATH, ...env }, encoding: 'utf8' })
  }
}

for (const config of ['config', 'GODOT_CONFIG']) test(`exports ${config} shell with literal launch arguments`, async t => {
  const { root, run } = await fixture(t)
  const args = ['--local-editor-preview', String.raw`--value=quoted "text" and \1`]
  const result = run({ EXPORT_CONFIG: config }, ...args)
  assert.equal(result.status, 0, result.stderr)
  const html = await readFile(path.join(root, 'build/web/index.html'), 'utf8')
  const value = JSON.parse(html.match(/= (\{.*\});/)[1])
  assert.deepEqual(value.args, args)
  assert.equal(value.renderingDriver, 'webgpu')
})

test('failed exports cannot silently reuse a stale artifact', async t => {
  const { root, run } = await fixture(t)
  const file = path.join(root, 'build/web/index.html')
  const stale = 'const config = {"args":["old"]};'
  await writeFile(file, stale)
  const result = run({ EXPORT_FAIL: '1' })
  assert.notEqual(result.status, 0)
  assert.match(result.stdout, /export failed/)
  assert.equal(await readFile(file, 'utf8'), stale)
})

test('script errors fail export even when the editor exits successfully', async t => {
  const { run } = await fixture(t)
  const result = run({ EXPORT_CONFIG: 'config', EXPORT_SCRIPT_ERROR: '1' })
  assert.notEqual(result.status, 0)
  assert.match(result.stdout, /SCRIPT ERROR/)
})
