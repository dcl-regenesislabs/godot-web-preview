import { execFileSync } from 'node:child_process'
import { appendFileSync, readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

export function needsBuild(eventName, event, changedFiles) {
  if (eventName === 'workflow_dispatch' || event.ref?.startsWith('refs/tags/')) return true
  if (!changedFiles) return true
  return changedFiles.some(file => !(
    ['README.md', 'AGENTS.md', 'CLAUDE.md', 'packages/runtime/README.md'].includes(file) ||
    (file.startsWith('docs/') && file.endsWith('.md'))
  ))
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const event = JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'))
  const base = event.pull_request?.base?.sha ?? event.before
  let files
  if (/^[a-f0-9]{40}$/.test(base ?? '') && !/^0+$/.test(base)) {
    try {
      files = execFileSync('git', ['diff', '--no-renames', '--name-only', '-z', base, 'HEAD'], { encoding: 'utf8' }).split('\0').filter(Boolean)
    } catch {
      console.warn('Could not compare the base revision; running the full build.')
    }
  }
  const build = needsBuild(process.env.GITHUB_EVENT_NAME, event, files)
  appendFileSync(process.env.GITHUB_OUTPUT, `build=${build}\n`)
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, build
    ? 'Full runtime build and validation requested.\n'
    : 'Only documentation changed; compilation and packaging are skipped. Unit tests and dependency audits still run.\n')
}
