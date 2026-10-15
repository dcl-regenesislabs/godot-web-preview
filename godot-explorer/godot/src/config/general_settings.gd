class_name GeneralSettings


static func apply_max_cache_size():
	if LocalPreviewRuntime.is_enabled():
		# Web filesystem caches occupy RAM, unlike the native disk cache.
		Global.content_provider.set_cache_folder_max_size(128 * 1024 * 1024)
		return
	var max_cache_size = 0
	match Global.get_config().max_cache_size:
		0:
			max_cache_size = 1024 * 1000 * 1000  # 1gb
		1:
			max_cache_size = 2048 * 1000 * 1000  # 2gb
		_:
			max_cache_size = 4096 * 1000 * 1000  # 4gb

	Global.content_provider.set_cache_folder_max_size(max_cache_size)
