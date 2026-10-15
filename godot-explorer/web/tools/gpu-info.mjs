import { chromium } from 'playwright';
import http from 'node:http';
const srv = http.createServer((q, r) => { r.writeHead(200, {'Content-Type':'text/html'}); r.end('<html></html>'); });
await new Promise(r => srv.listen(0, '127.0.0.1', r));
const headed = process.argv.includes('--headed');
const b = await chromium.launch({ channel: 'chrome', headless: !headed, args: ['--enable-unsafe-webgpu', '--enable-gpu', '--ignore-gpu-blocklist', '--enable-webgpu-developer-features'] });
const p = await b.newPage();
await p.goto(`http://127.0.0.1:${srv.address().port}/`);
console.log(await p.evaluate(async () => {
  const a = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!a) return 'no adapter';
  return JSON.stringify({ info: { vendor: a.info.vendor, arch: a.info.architecture, device: a.info.device, desc: a.info.description, fallback: a.info.isFallbackAdapter }, features: [...a.features].sort(), maxSampledTex: a.limits.maxSampledTexturesPerShaderStage, maxInterStage: a.limits.maxInterStageShaderVariables, maxStorageBuf: a.limits.maxStorageBuffersPerShaderStage }, null, 1);
}));
await b.close(); srv.close();
