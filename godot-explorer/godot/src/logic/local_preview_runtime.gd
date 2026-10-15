class_name LocalPreviewRuntime
extends Node

var _visibility_callback: JavaScriptObject
var _background := false
var _saved_fps := 30
var _saved_pause := false


static func is_enabled() -> bool:
	return (
		OS.has_feature("web")
		and (
			OS.get_cmdline_args().has("--local-editor-preview")
			or OS.get_cmdline_user_args().has("--local-editor-preview")
		)
	)


static func hide_controls(controls: Array[Control]) -> void:
	for control in controls:
		control.hide()
		control.process_mode = Node.PROCESS_MODE_DISABLED


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	_visibility_callback = JavaScriptBridge.create_callback(_visibility_changed)
	var browser := JavaScriptBridge.get_interface("window")
	browser.setLocalPreviewVisible = _visibility_callback


func _visibility_changed(args: Array) -> void:
	var background := not bool(args[0])
	if background == _background:
		return
	_background = background
	if background:
		_saved_fps = Engine.max_fps
		_saved_pause = get_tree().paused
		get_tree().paused = true
		Engine.max_fps = 5
		for action in InputMap.get_actions():
			Input.action_release(action)
	else:
		get_tree().paused = _saved_pause
		Engine.max_fps = _saved_fps
	RenderingServer.render_loop_enabled = not background
