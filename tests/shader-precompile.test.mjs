import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import test from 'node:test'

test('runtime shader compilation does not invoke optional native tools', () => {
  const result = spawnSync('python3', ['-c', `
import importlib.util
import tempfile
from pathlib import Path
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location('precompile', 'godotengine/drivers/webgpu/wgsl_precompile.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
env = Mock()
env.Dir.return_value = '/missing/host/toolchain'
env.get.return_value = False
with tempfile.TemporaryDirectory() as directory, patch.object(module.subprocess, 'run') as run:
    output = Path(directory) / 'generated.h'
    module.build_wgsl_precompiled([output], [], env)
    run.assert_not_called()
    assert '_wgsl_precompiled_count = 0;' in output.read_text()
`], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
})
