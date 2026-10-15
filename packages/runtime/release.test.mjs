import { it } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const { version } = JSON.parse(await readFile(new URL('./package.json', import.meta.url), 'utf8'))
const check = (tag) => execFileSync(process.execPath, [fileURLToPath(new URL('./scripts/check-release.mjs', import.meta.url))], {
  env: { ...process.env, RELEASE_TAG: tag }, stdio: 'pipe'
})

it('releases only the stable version named by the tag', () => {
  assert.doesNotThrow(() => check(version))
  for (const tag of ['main', '999.0.0', `v${version}`, `${version}-rc.1`, `${version}+build.1`, '$(touch injected)']) {
    assert.throws(() => check(tag), /Release tag must match/)
  }
})
