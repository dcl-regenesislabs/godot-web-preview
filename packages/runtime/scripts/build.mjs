import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'

const root = fileURLToPath(new URL('../', import.meta.url))
const workspace = path.resolve(root, '../..')
const source = path.resolve(process.argv[2] ?? path.join(workspace, 'godot-explorer/build/web'))
const required = ['index.html', 'index.js', 'index.wasm', 'index.side.wasm', 'index.pck', 'dclgodot.wasm', 'scene-comms.js', 'pulse.js', 'livekit-client.umd.js', 'livekit-LICENSE.txt']
const allowed = /^(index\.(html|js|wasm|side\.wasm|pck|png|icon\.png|apple-touch-icon\.png|audio\.[a-z.]+\.js)|dclgodot\.wasm|scene-comms\.js|pulse\.js|livekit-client\.umd\.js|livekit-LICENSE\.txt|libsentry\.[a-z0-9.]+\.wasm)$/
const names = (await fs.readdir(source)).filter(name => allowed.test(name)).sort()
for (const name of required) if (!names.includes(name)) throw new Error(`Missing runtime artifact: ${name}`)
const html = await fs.readFile(path.join(source, 'index.html'), 'utf8')
if (!html.includes('await initWebGPU()') || !html.includes('localPreviewReady')) throw new Error('Export does not support the local preview protocol.')
const git = (...args) => execFileSync('git', args, { cwd: workspace, encoding: 'utf8' }).trim()
const manifest = {
  version: JSON.parse(await fs.readFile(path.join(root, 'package.json'), 'utf8')).version,
  source: {
    repository: 'kuruk-mm/test-godot-web',
    revision: git('rev-parse', 'HEAD'),
    explorer: git('rev-parse', 'HEAD:godot-explorer'),
    engine: git('rev-parse', 'HEAD:godotengine')
  },
  files: {}
}
const dist = path.join(root, 'dist')
await fs.rm(dist, { recursive: true, force: true })
await fs.mkdir(dist, { recursive: true })
for (const name of names) {
  const file = path.join(source, name)
  if (!(await fs.lstat(file)).isFile()) throw new Error(`Runtime artifact must be a regular file: ${name}`)
  const bytes = await fs.readFile(file)
  await fs.writeFile(path.join(dist, name), bytes)
  manifest.files[name] = { bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') }
}
await fs.writeFile(path.join(dist, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n')
console.log(`Prepared runtime ${manifest.version} from ${manifest.source.revision}`)
