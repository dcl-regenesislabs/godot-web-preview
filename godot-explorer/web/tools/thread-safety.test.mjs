import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

test('content guards accept main and worker threads without engine errors', () => {
  const project = mkdtempSync(path.join(tmpdir(), 'preview-thread-safety-'))
  const root = new URL('../../', import.meta.url)
  try {
    copyFileSync(new URL('godot/src/logic/thread_safety.gd', root), path.join(project, 'safety.gd'))
    writeFileSync(path.join(project, 'project.godot'), 'config_version=5\n')
    writeFileSync(path.join(project, 'check.gd'), `extends SceneTree

const Safety = preload("res://safety.gd")

func _initialize() -> void:
	Safety.set_thread_safety_checks_enabled(false)
	Safety.set_thread_safety_checks_enabled(true)
	var worker := Thread.new()
	worker.start(func():
		Safety.set_thread_safety_checks_enabled(false)
		Safety.set_thread_safety_checks_enabled(true)
	)
	worker.wait_to_finish()
	print("THREAD_SAFETY_PASS")
	quit(0)
`)
    const executable = fileURLToPath(new URL('.bin/godot/godot4_bin', root))
    const result = spawnSync(executable, ['--headless', '--path', project, '--script', 'res://check.gd'], {
      encoding: 'utf8', timeout: 30_000, stdio: ['ignore', 'pipe', 'pipe']
    })
    const output = result.stdout + result.stderr
    assert.equal(result.status, 0, result.error?.message ?? output)
    assert.match(output, /THREAD_SAFETY_PASS/)
    assert.doesNotMatch(output, /ERROR:/)
  } finally {
    rmSync(project, { recursive: true, force: true })
  }
})
