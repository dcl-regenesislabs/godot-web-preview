// Serve an exported web build on loopback: node serve-web.mjs <build_dir> [port].
import http from 'node:http'
import { createReadStream } from 'node:fs'
import { realpath, stat } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.pck': 'application/octet-stream', '.png': 'image/png', '.json': 'application/json' }
const outside = (root, file) => {
  const relative = path.relative(root, file)
  return relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)
}

export function serveBuild(directory, { isolate = false } = {}) {
  const root = realpath(directory)
  return http.createServer(async (req, res) => {
    if (isolate) {
      res.setHeader('Cross-Origin-Opener-Policy', 'same-origin')
      res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp')
    }
    res.setHeader('X-Content-Type-Options', 'nosniff')
    res.setHeader('Referrer-Policy', 'no-referrer')
    if (!['GET', 'HEAD'].includes(req.method)) { res.writeHead(405); res.end(); return }
    try {
      const base = await root
      const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname)
      const file = path.resolve(base, '.' + (pathname === '/' ? '/index.html' : pathname))
      if (outside(base, file)) { res.writeHead(403); res.end(); return }
      const canonical = await realpath(file)
      if (outside(base, canonical)) { res.writeHead(403); res.end(); return }
      const info = await stat(canonical)
      if (!info.isFile()) { res.writeHead(404); res.end(); return }
      res.writeHead(200, { 'Content-Type': MIME[path.extname(canonical)] || 'application/octet-stream', 'Content-Length': info.size, 'Cache-Control': 'no-store' })
      if (req.method === 'HEAD') { res.end(); return }
      const stream = createReadStream(canonical)
      res.on('close', () => stream.destroy())
      stream.on('error', () => res.destroy())
      stream.pipe(res)
    } catch (error) {
      res.writeHead(error instanceof URIError ? 400 : error.code === 'ENOENT' ? 404 : 500)
      res.end()
    }
  })
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [directory, port = '8060'] = process.argv.slice(2)
  if (!directory) throw new Error('Usage: node serve-web.mjs <build_dir> [port]')
  serveBuild(directory).listen(Number(port), '127.0.0.1', () => console.log(`http://localhost:${port}/`))
}
