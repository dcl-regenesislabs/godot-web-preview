extends Node3D

var _frames := 0
var _elapsed := 0.0

func _ready() -> void:
	if ClassDB.class_exists("HelloWeb"):
		var h = ClassDB.instantiate("HelloWeb")
		print("SMOKE rust: ", h.greet("decentraland"))
		h.free()
	else:
		print("SMOKE rust: HelloWeb class missing")
	print("SMOKE driver=", RenderingServer.get_current_rendering_driver_name(),
		" method=", RenderingServer.get_current_rendering_method(),
		" adapter=", RenderingServer.get_video_adapter_name(),
		" vendor=", RenderingServer.get_video_adapter_vendor())

func _process(delta: float) -> void:
	$Pivot.rotate_y(delta)
	_frames += 1
	_elapsed += delta
	if _elapsed >= 2.0:
		print("SMOKE fps=", snappedf(_frames / _elapsed, 0.1))
		$Label.text = "%s / %s  %.1f fps" % [RenderingServer.get_current_rendering_driver_name(), RenderingServer.get_current_rendering_method(), _frames / _elapsed]
		_frames = 0
		_elapsed = 0.0
