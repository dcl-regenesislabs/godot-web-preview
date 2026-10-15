import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, symlink, rm } from 'node:fs/promises'
import { once } from 'node:events'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { serveBuild } from '../godot-explorer/web/tools/serve-web.mjs'

test('serves build files while rejecting traversal, symlinks and malformed paths', async t => {
  const root = await mkdtemp(path.join(tmpdir(), 'preview-server-'))
  t.after(() => rm(root, { recursive: true, force: true }))
  const build = path.join(root, 'build')
  await mkdir(build)
  await writeFile(path.join(build, 'index.html'), 'fixture')
  await writeFile(path.join(root, 'private.txt'), 'outside the export')
  await symlink(path.join(root, 'private.txt'), path.join(build, 'link.txt'))
  const server = serveBuild(build)
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  t.after(() => new Promise(resolve => { server.closeAllConnections(); server.close(resolve) }))
  const base = `http://127.0.0.1:${server.address().port}`
  assert.equal(await (await fetch(base)).text(), 'fixture')
  const head = await fetch(base, { method: 'HEAD' })
  assert.equal(head.headers.get('Content-Length'), '7')
  assert.equal(await head.text(), '')
  for (const pathname of ['/..%2fprivate.txt', '/link.txt']) assert.equal((await fetch(base + pathname)).status, 403)
  assert.equal((await fetch(base + '/%ZZ')).status, 400)
  assert.equal((await fetch(base, { method: 'POST' })).status, 405)
  assert.equal((await fetch(base + '/missing')).status, 404)
})
