import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { needsBuild } from '../scripts/ci-scope.mjs'

async function fixture(t) {
  const root = await mkdtemp(path.join(tmpdir(), 'preview build '))
  t.after(() => rm(root, { recursive: true, force: true }))
  for (const dir of ['bin', 'scripts', 'godot-explorer/lib', 'godot-explorer/.bin/protoc/bin']) {
    await mkdir(path.join(root, dir), { recursive: true })
  }
  await copyFile(new URL('../build.sh', import.meta.url), path.join(root, 'build.sh'))
  await copyFile(new URL('../scripts/check-native.sh', import.meta.url), path.join(root, 'scripts/check-native.sh'))
  const log = path.join(root, 'commands.log')
  const env = { ...process.env, PATH: path.join(root, 'bin') + path.delimiter + process.env.PATH, BUILD_LOG: log }
  return {
    root,
    write: (name, body) => writeFile(path.join(root, name), `#!/bin/bash\nset -eu\n${body}\n`, { mode: 0o755 }),
    run: (script, args = [], overrides = {}) => spawnSync('/bin/bash', [path.join(root, script), ...args], {
      env: { ...env, JOBS: '', BUILD_PRIORITY: '', ...overrides }, encoding: 'utf8'
    }),
    log: () => readFile(log, 'utf8')
  }
}

test('local builds stay throttled and CI can use normal priority and all runner cores', async t => {
  const f = await fixture(t)
  await f.write('bin/taskpolicy', 'echo background >> "$BUILD_LOG"; shift 2; exec "$@"')
  await f.write('bin/nice', 'echo nice >> "$BUILD_LOG"; shift 2; exec "$@"')
  await f.write('bin/cargo', 'echo "cargo jobs=$CARGO_BUILD_JOBS $*" >> "$BUILD_LOG"')
  const local = f.run('build.sh', ['native'])
  assert.equal(local.status, 0, local.stderr)
  assert.equal(await f.log(), 'background\nnice\ncargo jobs=2 run -- install --no-templates\nbackground\nnice\ncargo jobs=2 run -- build --no-default-features\n')
  await writeFile(path.join(f.root, 'commands.log'), '')
  const ci = f.run('build.sh', ['native'], { BUILD_PRIORITY: 'normal', JOBS: '3' })
  assert.equal(ci.status, 0, ci.stderr)
  assert.equal(await f.log(), 'cargo jobs=3 run -- install --no-templates\ncargo jobs=3 run -- build --no-default-features\n')
})

test('invalid build settings fail before starting a compiler', async t => {
  const f = await fixture(t)
  for (const overrides of [{ JOBS: '0' }, { JOBS: '-j8' }, { BUILD_PRIORITY: 'invalid' }]) {
    assert.equal(f.run('build.sh', ['native'], overrides).status, 1)
  }
})

test('native compatibility checks find the downloaded protoc on a clean runner', async t => {
  const f = await fixture(t)
  await f.write('godot-explorer/.bin/protoc/bin/protoc', 'echo fixture-protoc')
  await f.write('bin/cargo', 'test "$(protoc --version)" = fixture-protoc; echo "$*" >> "$BUILD_LOG"')
  const result = f.run('scripts/check-native.sh', [], { JOBS: '3' })
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /fixture-protoc/)
  assert.equal((await f.log()).trim().split('\n').length, 3)
  assert.match(await f.log(), /check --locked --no-default-features --features use_deno -j 3/)
})

test('only explicitly allowed documentation changes skip compilation', () => {
  const docs = ['README.md', 'AGENTS.md', 'CLAUDE.md', 'docs/UPSTREAMS.md', 'packages/runtime/README.md']
  assert.equal(needsBuild('pull_request', {}, docs), false)
  assert.equal(needsBuild('push', { ref: 'refs/heads/main' }, docs), false)
  for (const file of ['build.sh', '.github/workflows/preview.yml', 'godotengine/README.md', 'godot-explorer/godot/main.tscn', 'packages/runtime/package.json', 'docs/helper.mjs']) {
    assert.equal(needsBuild('pull_request', {}, [...docs, file]), true, file)
  }
  assert.equal(needsBuild('workflow_dispatch', {}, docs), true)
  assert.equal(needsBuild('push', { ref: 'refs/tags/0.1.0' }, docs), true)
  assert.equal(needsBuild('push', {}, undefined), true)
})
