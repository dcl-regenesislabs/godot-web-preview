static func set_thread_safety_checks_enabled(enabled: bool) -> void:
	if not Thread.is_main_thread():
		Thread.set_thread_safety_checks_enabled(enabled)
