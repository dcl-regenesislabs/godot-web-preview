import { afterEach, it } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { planUpdates, syncUpdates } from '../scripts/upstream.mjs'

const directories = []
afterEach(() => { for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true }) })
const git = (root, ...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()

function initialize(root, branch) {
  mkdirSync(root)
  git(root, 'init', '-b', branch)
  git(root, 'config', 'user.name', 'Preview test')
  git(root, 'config', 'user.email', 'preview-test@example.invalid')
  git(root, 'config', 'commit.gpgsign', 'false')
  writeFileSync(path.join(root, 'README.md'), 'fixture\n')
  commit(root, 'Initialize fixture')
}

function commit(root, message) {
  git(root, 'add', '.')
  git(root, 'commit', '-m', message)
  return git(root, 'rev-parse', 'HEAD')
}

function constants(pin, version = '4.6.2') {
  return `pub const GODOT_CURRENT_VERSION: &str = "${version}";\npub const GODOT_BUILD_SHA: &str = "${pin.slice(0, 9)}";\npub const GODOT_USE_BRANCH: Option<&str> = None;\n`
}

function fixture() {
  const directory = mkdtempSync(path.join(tmpdir(), 'preview-subtrees-'))
  directories.push(directory)
  const engine = path.join(directory, 'engine')
  const explorer = path.join(directory, 'explorer')
  const root = path.join(directory, 'preview')
  initialize(engine, '4.6.2')
  const engineBase = git(engine, 'rev-parse', 'HEAD')
  initialize(explorer, 'main')
  mkdirSync(path.join(explorer, 'src'))
  writeFileSync(path.join(explorer, 'src/consts.rs'), constants(engineBase))
  writeFileSync(path.join(explorer, 'input.txt'), 'original input\n')
  const explorerBase = commit(explorer, 'Pin the mobile engine')
  initialize(root, 'chore/test-sync')
  for (const [prefix, source, revision] of [['godotengine', engine, engineBase], ['godot-explorer', explorer, explorerBase]]) {
    git(root, 'subtree', 'add', `--prefix=${prefix}`, source, revision, '--squash')
  }
  const lock = { version: 1, sources: {
    engine: { path: 'godotengine', repository: engine, branch: '4.6.2', revision: engineBase },
    explorer: { path: 'godot-explorer', repository: explorer, branch: 'main', revision: explorerBase }
  } }
  writeFileSync(path.join(root, 'upstreams.json'), JSON.stringify(lock, null, 2) + '\n')
  writeFileSync(path.join(root, 'godot-explorer/browser.txt'), 'browser support\n')
  commit(root, 'Keep browser changes separate from the imported baseline')
  return { directory, root, engine, explorer, engineBase, explorerBase }
}

it('updates the mobile pair from a fresh clone and preserves browser changes', () => {
  const state = fixture()
  const root = path.join(state.directory, 'clone')
  git(state.directory, 'clone', '--no-local', state.root, root)
  git(root, 'config', 'user.name', 'Preview test')
  git(root, 'config', 'user.email', 'preview-test@example.invalid')
  git(root, 'config', 'commit.gpgsign', 'false')
  writeFileSync(path.join(state.engine, 'fix.txt'), 'engine fix\n')
  const engine = commit(state.engine, 'Fix rendering')
  writeFileSync(path.join(state.explorer, 'src/consts.rs'), constants(engine))
  writeFileSync(path.join(state.explorer, 'scene-api.txt'), 'new API\n')
  const explorer = commit(state.explorer, 'Use the new engine')
  const plan = syncUpdates(root)
  assert.equal(plan.updates.length, 2)
  assert.equal(readFileSync(path.join(root, 'godot-explorer/browser.txt'), 'utf8'), 'browser support\n')
  assert.equal(readFileSync(path.join(root, 'godot-explorer/scene-api.txt'), 'utf8'), 'new API\n')
  assert.equal(readFileSync(path.join(root, 'godotengine/fix.txt'), 'utf8'), 'engine fix\n')
  const lock = JSON.parse(readFileSync(path.join(root, 'upstreams.json'), 'utf8'))
  assert.equal(lock.sources.engine.revision, engine)
  assert.equal(lock.sources.explorer.revision, explorer)
  assert.deepEqual(syncUpdates(root).updates, [])
  assert.equal(git(root, 'status', '--porcelain'), '')
})

it('does not import an engine update before the mobile app adopts it', () => {
  const { root, engine } = fixture()
  writeFileSync(path.join(engine, 'future.txt'), 'not adopted\n')
  commit(engine, 'Prepare a future engine update')
  assert.deepEqual(planUpdates(root).updates, [])
})

it('stops at conflicting changes and keeps the previous imported revision', () => {
  const { root, explorer, explorerBase } = fixture()
  writeFileSync(path.join(root, 'godot-explorer/input.txt'), 'browser input\n')
  commit(root, 'Adapt browser input')
  writeFileSync(path.join(explorer, 'input.txt'), 'new mobile input\n')
  commit(explorer, 'Rewrite mobile input')
  assert.throws(() => syncUpdates(root))
  assert.match(git(root, 'diff', '--name-only', '--diff-filter=U'), /godot-explorer\/input.txt/)
  assert.equal(JSON.parse(readFileSync(path.join(root, 'upstreams.json'), 'utf8')).sources.explorer.revision, explorerBase)
  git(root, 'merge', '--abort')
  assert.equal(readFileSync(path.join(root, 'godot-explorer/input.txt'), 'utf8'), 'browser input\n')
  assert.throws(() => syncUpdates(root))
  writeFileSync(path.join(root, 'godot-explorer/input.txt'), 'combined browser and mobile input\n')
  commit(root, 'Resolve input changes')
  writeFileSync(path.join(explorer, 'next-api.txt'), 'another upstream change\n')
  commit(explorer, 'Advance upstream while the conflict is being resolved')
  syncUpdates(root)
  assert.equal(JSON.parse(readFileSync(path.join(root, 'upstreams.json'), 'utf8')).sources.explorer.revision,
    git(explorer, 'rev-parse', 'HEAD'))
  assert.equal(readFileSync(path.join(root, 'godot-explorer/input.txt'), 'utf8'), 'combined browser and mobile input\n')
})

it('requires review for a new engine version and refuses dirty or protected branches', () => {
  const { root, explorer, engineBase } = fixture()
  writeFileSync(path.join(explorer, 'src/consts.rs'), constants(engineBase, '4.7'))
  commit(explorer, 'Upgrade the mobile engine')
  assert.throws(() => planUpdates(root), /engine version or branch override changed/)
  git(root, 'checkout', '-b', 'main')
  assert.throws(() => syncUpdates(root), /Create a feat/)
  git(root, 'checkout', 'chore/test-sync')
  writeFileSync(path.join(root, 'local.txt'), 'uncommitted\n')
  assert.throws(() => syncUpdates(root), /Commit or stash/)
})

it('rejects updates after a PR squash discards the imported subtree history', () => {
  const { root, explorer } = fixture()
  const start = git(root, 'rev-parse', 'HEAD')
  writeFileSync(path.join(explorer, 'new-api.txt'), 'first update\n')
  commit(explorer, 'Add a scene API')
  syncUpdates(root)
  git(root, 'checkout', '-b', 'chore/squashed-import', start)
  git(root, 'merge', '--squash', 'chore/test-sync')
  commit(root, 'Squash the upstream update')
  writeFileSync(path.join(explorer, 'new-api.txt'), 'second update\n')
  commit(explorer, 'Update the scene API')
  assert.throws(() => syncUpdates(root), /subtree history does not match/)
  assert.equal(git(root, 'status', '--porcelain'), '')
})
