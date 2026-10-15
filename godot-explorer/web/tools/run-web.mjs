// Serve an exported Godot web build and drive it in headless Chrome.
// usage: node run-web.mjs <build_dir> <seconds> <screenshot.png> [--headed]
import assert from 'node:assert/strict';
import { serveBuild } from './serve-web.mjs';
import { chromium } from 'playwright';

const [dir, secs = '20', shot = 'shot.png'] = process.argv.slice(2);
const headed = process.argv.includes('--headed');
const isolate = process.argv.includes('--isolate');
const ci = process.argv.includes('--ci');
const server = serveBuild(dir, { isolate });
await new Promise(r => server.listen(0, '127.0.0.1', r));
const url = `http://localhost:${server.address().port}/index.html`;

let browser;
try {
  browser = await chromium.launch({
    channel: ci ? 'chromium' : 'chrome',
    headless: !headed,
    args: ['--enable-unsafe-webgpu', '--enable-gpu', '--ignore-gpu-blocklist',
      ...(ci && process.platform === 'linux' ? ['--enable-features=Vulkan', '--use-angle=vulkan', '--use-vulkan=swiftshader', '--use-webgpu-adapter=swiftshader', '--disable-vulkan-surface', '--disable-gpu-watchdog'] : [])],
  });
  // --size=1920x1080 --dpr=2 to mimic a fullscreen Retina window
  const argOf = (name, dflt) => (process.argv.find(a => a.startsWith(`--${name}=`)) || `=${dflt}`).split('=')[1];
  const [vw, vh] = argOf('size', '1280x720').split('x').map(Number);
  const page = await browser.newPage({ viewport: { width: vw, height: vh }, deviceScaleFactor: Number(argOf('dpr', '1')) });
  const t0 = Date.now();
  const gpuErrors = [];
  const engineErrors = [];
  let rendererReady = false;
  page.on('console', m => {
    console.log(`[${((Date.now() - t0) / 1000).toFixed(1)}s][${m.type()}] ${m.text()}`);
    if (/WebGPU.*(uncaptured error|device lost)/i.test(m.text())) gpuErrors.push(m.text());
    if (/^(SCRIPT )?ERROR:/.test(m.text())) engineErrors.push(m.text());
    if (/WebGPU 1\.0 -/.test(m.text())) rendererReady = true;
  });
  page.on("response", r => { if (r.status() >= 400) console.log(`[http ${r.status()}] ${r.request().method()} ${r.url().slice(0, 200)} ${(r.request().postData() || "").slice(0, 300)}`); });
  page.on('requestfailed', r => console.log(`[reqfail] ${r.url().slice(0, 160)} ${r.failure()?.errorText}`));
  const pageErrors = [];
  page.on('pageerror', e => { pageErrors.push(e.message); console.log(`[pageerror] ${e.message}`); });
  await page.goto(url);
  console.log(`[gpu] API available: ${await page.evaluate(() => Boolean(navigator.gpu))}`);
  // --actions "8:click:640,497;20:shot:a.png;25:key:Enter" (seconds since load)
  const actionsArg = process.argv.find(a => a.startsWith('--actions='));
  const actions = actionsArg ? actionsArg.slice(10).split(';').filter(Boolean).map(a => {
    const [t, kind, ...rest] = a.split(':');
    const arg = rest.join(':');
    return { t: Number(t), kind, arg };
  }).sort((a, b) => a.t - b.t) : [];
  for (const a of actions) {
    const wait = a.t * 1000 - (Date.now() - t0);
    if (wait > 0) await page.waitForTimeout(wait);
    console.log(`[action ${a.t}s] ${a.kind} ${a.arg}`);
    if (a.kind === 'click') { const [x, y] = a.arg.split(',').map(Number); await page.mouse.click(x, y); }
    else if (a.kind === 'shot') await page.screenshot({ path: a.arg });
    else if (a.kind === 'key') await page.keyboard.press(a.arg);
    else if (a.kind === 'eval') console.log(`[eval] ${JSON.stringify(await page.evaluate(a.arg))}`);
  }
  const assertion = argOf('assert', '');
  if (assertion) {
    assert.match(assertion, /^[A-Za-z][A-Za-z0-9]*$/);
    await page.waitForFunction(name => window[name] !== undefined, assertion, { timeout: Number(secs) * 1000 });
    const result = await page.evaluate(name => window[name], assertion);
    console.log(`[result] ${assertion}: ${JSON.stringify(result)}`);
    await page.evaluate(async timeout => {
      const device = window.godotEngine?.config.preinitializedWebGPUDevice;
      if (!device) throw new Error('The fixture has no WebGPU device');
      await Promise.race([
        device.queue.onSubmittedWorkDone(),
        new Promise((_, reject) => setTimeout(() => reject(new Error('GPU work did not finish')), timeout)),
      ]);
    }, Number(secs) * 1000);
    await page.screenshot({ path: shot });
    assert.ok(result === 'PASS' || result?.passed === true, `${assertion} failed: ${JSON.stringify(result)}`);
    assert.deepEqual(pageErrors, [], 'Browser errors during regression test');
    assert.deepEqual(gpuErrors, [], 'WebGPU errors during regression test');
    assert.deepEqual(engineErrors, [], 'Engine errors during regression test');
    assert.ok(rendererReady, 'The fixture did not initialize the WebGPU renderer');
  } else {
    const remaining = Number(secs) * 1000 - (Date.now() - t0);
    if (remaining > 0) await page.waitForTimeout(remaining);
    await page.screenshot({ path: shot });
  }
  console.log(`[done] screenshot ${shot}`);
} finally {
  await browser?.close();
  await new Promise(resolve => server.close(resolve));
}
