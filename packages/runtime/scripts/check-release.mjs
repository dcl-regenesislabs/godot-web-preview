import { readFile } from 'node:fs/promises'

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'))
if (!/^\d+\.\d+\.\d+$/.test(version) || process.env.RELEASE_TAG !== `v${version}`) {
  throw new Error(`Release tag must match the stable package version v${version}`)
}
