extends Node

var _browser: JavaScriptObject


func _process(_delta: float) -> void:
	if _browser != null:
		_browser.framePacingEngineFrames = Engine.get_process_frames()


func _ready() -> void:
	_async_measure()


func _async_measure() -> void:
	Engine.max_fps = 5
	if not OS.has_feature("web"):
		return
	_browser = JavaScriptBridge.get_interface("window")
	for frame in range(12):
		await get_tree().process_frame
	(
		JavaScriptBridge
		. eval(
			"""
		setTimeout(() => {
			const start = performance.now();
			const engineStart = window.framePacingEngineFrames;
			let frames = 0;
			const tick = () => {
				frames++;
				const elapsed = performance.now() - start;
				if (elapsed < 5000) return requestAnimationFrame(tick);
				const fps = frames * 1000 / elapsed;
				const engineFps = (window.framePacingEngineFrames - engineStart) * 1000 / elapsed;
				const passed = fps > 30 && engineFps > 0 && engineFps < 8;
				window.framePacingResult = { passed, fps, engineFps };
				console.log('[FramePacing]', passed ? 'PASS' : 'FAIL',
					JSON.stringify(window.framePacingResult));
			};
			requestAnimationFrame(tick);
		}, 1000);
	"""
		)
	)
