extends Node3D

var failures := 0


func _ready() -> void:
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 4
	camera.position.z = 5
	add_child(camera)
	var light := DirectionalLight3D.new()
	add_child(light)
	test_image_texture()
	test_gltf_roundtrip()
	var result := "PASS" if failures == 0 else "FAIL (%d)" % failures
	print("[TextureRoundtrip] ", result)
	var label := Label.new()
	label.text = (
		"Texture round trip: " + result + "\nLeft: imported glTF. Right: saved and reloaded glTF."
	)
	label.position = Vector2(20, 20)
	add_child(label)
	if OS.has_feature("web"):
		JavaScriptBridge.eval("window.textureRoundtripResult = " + JSON.stringify(result))


func check(condition: bool, message: String) -> void:
	if not condition:
		failures += 1
		push_error("[TextureRoundtrip] " + message)


func test_image_texture() -> void:
	for format in [Image.FORMAT_RGBA8, Image.FORMAT_RGB8, Image.FORMAT_L8, Image.FORMAT_LA8]:
		var source := Image.create(8, 8, false, format)
		source.fill(Color(0.2, 0.8, 0.4, 0.6))
		source.generate_mipmaps()
		var expected := source.get_data()
		var texture := ImageTexture.create_from_image(source)
		source.fill(Color.BLACK)
		var readback := texture.get_image()
		check(readback != null, "Immediate readback is missing for format %d" % format)
		if readback == null:
			continue
		check(
			readback.get_format() == format and readback.has_mipmaps(), "Format or mipmaps changed"
		)
		check(readback.get_data() == expected, "Mutating the input changed texture data")
		readback.fill(Color.RED)
		check(texture.get_image().get_data() == expected, "Mutating readback changed texture data")
		var proxy := RenderingServer.texture_proxy_create(texture.get_rid())
		texture.update(source)
		check(texture.get_image().get_data() == source.get_data(), "Update returned stale data")
		check(
			RenderingServer.texture_2d_get(proxy).get_data() == source.get_data(),
			"Proxy returned stale data"
		)
		RenderingServer.free_rid(proxy)
		var replacement := Image.create(4, 4, false, format)
		replacement.fill(Color.WHITE)
		texture.set_image(replacement)
		check(
			texture.get_image().get_data() == replacement.get_data(),
			"Replacement returned stale data"
		)


func test_gltf_roundtrip() -> void:
	var document := GLTFDocument.new()
	var state := GLTFState.new()
	var error := document.append_from_buffer(
		FileAccess.get_file_as_bytes("res://checker.gltf.json"), "res://", state
	)
	check(error == OK, "glTF import failed")
	if error != OK:
		return
	var original := document.generate_scene(state)
	var packed := PackedScene.new()
	check(packed.pack(original) == OK, "Scene packing failed")
	check(ResourceSaver.save(packed, "user://roundtrip.scn") == OK, "Scene saving failed")
	var saved := (
		ResourceLoader.load("user://roundtrip.scn", "PackedScene", ResourceLoader.CACHE_MODE_IGNORE)
		as PackedScene
	)
	var restored := saved.instantiate()
	var mesh := restored.find_children("*", "MeshInstance3D")[0] as MeshInstance3D
	var material := mesh.mesh.surface_get_material(0) as StandardMaterial3D
	var image := material.albedo_texture.get_image()
	check(image != null, "Saved glTF lost its texture pixels")
	if image != null:
		check(image.get_size() == Vector2i(2, 2), "Saved glTF texture dimensions changed")
		check(
			image.get_pixel(0, 0).is_equal_approx(Color8(32, 192, 64)),
			"Saved glTF texture color changed"
		)
	original.position.x = -1.4
	restored.position.x = 1.4
	add_child(original)
	add_child(restored)
