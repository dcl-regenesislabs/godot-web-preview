import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const paths = { engine: 'godotengine', explorer: 'godot-explorer' }
const shaPattern = /^[a-f0-9]{40}$/

function git(root, ...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()
}

function ancestor(root, before, after) {
  try {
    git(root, 'merge-base', '--is-ancestor', before, after)
    return true
  } catch (error) {
    if (error.status === 1) return false
    throw error
  }
}

export function readUpstreams(root) {
  const lock = JSON.parse(readFileSync(path.join(root, 'upstreams.json'), 'utf8'))
  if (lock.version !== 1 || Object.keys(lock.sources ?? {}).sort().join(',') !== 'engine,explorer') {
    throw new Error('Unsupported upstreams.json format')
  }
  for (const [name, prefix] of Object.entries(paths)) {
    const source = lock.sources[name]
    if (source.path !== prefix || !shaPattern.test(source.revision) ||
        typeof source.repository !== 'string' || !source.repository || source.repository.startsWith('-')) {
      throw new Error(`Invalid ${name} source`)
    }
    git(root, 'check-ref-format', `refs/heads/${source.branch}`)
  }
  return lock
}

export function planUpdates(root) {
  const lock = readUpstreams(root)
  const explorerSource = lock.sources.explorer
  const advertised = git(root, 'ls-remote', '--exit-code', '--', explorerSource.repository,
    `refs/heads/${explorerSource.branch}`).split(/\s/)[0]
  if (!shaPattern.test(advertised)) throw new Error('Invalid explorer upstream revision')
  if (advertised === explorerSource.revision) return { updates: [] }
  const tips = {}
  for (const [name, source] of Object.entries(lock.sources)) {
    const ref = `refs/remotes/upstream/${name}`
    git(root, 'fetch', '--no-tags', '--no-recurse-submodules', '--', source.repository,
      `+refs/heads/${source.branch}:${ref}`)
    tips[name] = git(root, 'rev-parse', `${ref}^{commit}`)
    if (!ancestor(root, source.revision, tips[name])) {
      throw new Error(`${name} upstream no longer contains the imported baseline; review its history before syncing`)
    }
  }

  const constants = git(root, 'show', `${tips.explorer}:src/consts.rs`)
  const version = constants.match(/pub const GODOT_CURRENT_VERSION: &str = "([0-9.]+)";/)?.[1]
  const pin = constants.match(/pub const GODOT_BUILD_SHA: &str = "([a-f0-9]{7,40})";/)?.[1]
  if (version !== lock.sources.engine.branch || !pin ||
      !/pub const GODOT_USE_BRANCH: Option<&str> = None;/.test(constants)) {
    throw new Error('The mobile engine version or branch override changed; review the toolchain before syncing')
  }
  const mobileEngine = git(root, 'rev-parse', '--verify', `${pin}^{commit}`)
  if (!ancestor(root, mobileEngine, tips.engine)) throw new Error('The mobile engine pin is outside the tracked engine branch')
  const currentEngine = lock.sources.engine.revision
  let engine = mobileEngine
  if (ancestor(root, mobileEngine, currentEngine)) engine = currentEngine
  else if (!ancestor(root, currentEngine, mobileEngine)) throw new Error('The mobile engine pin diverged from our baseline')

  return {
    mobileEngine,
    updates: Object.entries({ engine, explorer: tips.explorer }).map(([name, revision]) => ({
      name, path: paths[name], previous: lock.sources[name].revision, revision
    })).filter(update => update.previous !== update.revision)
  }
}

export function syncUpdates(root) {
  const branch = git(root, 'symbolic-ref', '--short', 'HEAD')
  if (!/^(feat|fix|chore)\//.test(branch)) throw new Error('Create a feat/, fix/ or chore/ branch before syncing')
  if (git(root, 'status', '--porcelain')) throw new Error('Commit or stash local changes before syncing')
  const plan = planUpdates(root)
  const lock = readUpstreams(root)
  for (const update of plan.updates) {
    const message = git(root, 'log', '-1', '--format=%B', `--grep=^git-subtree-dir: ${update.path}/*$`, 'HEAD')
    const imported = message.match(/^git-subtree-split: ([a-f0-9]{40})$/m)?.[1]
    if (!imported || !ancestor(root, update.previous, imported) || !ancestor(root, imported, update.revision)) {
      throw new Error(`${update.name} subtree history does not match upstreams.json; preserve import commits when merging sync PRs`)
    }
    git(root, 'subtree', 'merge', `--prefix=${update.path}`, '--squash', update.revision,
      '-m', `Sync ${update.name} with the mobile upstream`)
    lock.sources[update.name].revision = update.revision
    writeFileSync(path.join(root, 'upstreams.json'), JSON.stringify(lock, null, 2) + '\n')
    git(root, 'add', 'upstreams.json')
    git(root, 'commit', '-m', `Record the imported ${update.name} revision`)
  }
  return plan
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = fileURLToPath(new URL('../', import.meta.url))
  try {
    const command = process.argv[2]
    if (command !== 'check' && command !== 'sync') throw new Error('Usage: node scripts/upstream.mjs check|sync')
    console.log(JSON.stringify(command === 'check' ? planUpdates(root) : syncUpdates(root), null, 2))
  } catch (error) {
    console.error(error.stderr?.toString().trim() || error.message)
    process.exitCode = 1
  }
}
