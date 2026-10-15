import { readFile } from 'node:fs/promises'

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'))
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version) || process.env.RELEASE_TAG !== version) {
  throw new Error(`Release tag must match the stable package version ${version} without a v prefix`)
}
