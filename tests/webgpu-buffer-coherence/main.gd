extends Node3D

var failures := 0
var rd: RenderingDevice


func check(condition: bool, message: String) -> void:
	if not condition:
		failures += 1
		push_error("[BufferCoherence] " + message)


func dispatch(pipeline: RID, uniforms: RID) -> void:
	var commands := rd.compute_list_begin()
	rd.compute_list_bind_compute_pipeline(commands, pipeline)
	rd.compute_list_bind_uniform_set(commands, uniforms, 0)
	rd.compute_list_dispatch(commands, 1, 1, 1)
	rd.compute_list_end()


func _ready() -> void:
	_async_check_buffers()


func _async_check_buffers() -> void:
	rd = RenderingServer.get_rendering_device()
	var initial := PackedInt32Array([1, 2, 3, 4]).to_byte_array()
	var vertex := rd.vertex_buffer_create(
		initial.size(), initial, RenderingDevice.BUFFER_CREATION_AS_STORAGE_BIT
	)
	check(rd.buffer_get_data(vertex) == initial, "Initial vertex data changed")
	var replacement := PackedInt32Array([9, 8, 7, 6]).to_byte_array()
	rd.buffer_update(vertex, 0, replacement.size(), replacement)
	check(rd.buffer_get_data(vertex) == replacement, "CPU update returned stale data")
	rd.buffer_clear(vertex, 4, 4)
	var cleared := PackedInt32Array([9, 0, 7, 6]).to_byte_array()
	check(rd.buffer_get_data(vertex) == cleared, "Clear returned stale data")
	var copied := rd.vertex_buffer_create(16, initial)
	rd.buffer_copy(vertex, copied, 0, 0, 16)
	check(rd.buffer_get_data(copied) == cleared, "Copy returned stale data")

	var reader := rd.shader_create_from_spirv(load("res://read.glsl").get_spirv())
	var writer := rd.shader_create_from_spirv(load("res://write.glsl").get_spirv())
	var read_pipeline := rd.compute_pipeline_create(reader)
	var write_pipeline := rd.compute_pipeline_create(writer)
	var output := rd.storage_buffer_create(4, PackedInt32Array([0]).to_byte_array())
	var input_uniform := RDUniform.new()
	input_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	input_uniform.binding = 0
	input_uniform.add_id(vertex)
	var output_uniform := RDUniform.new()
	output_uniform.uniform_type = RenderingDevice.UNIFORM_TYPE_STORAGE_BUFFER
	output_uniform.binding = 1
	output_uniform.add_id(output)
	var uniforms := rd.uniform_set_create([input_uniform, output_uniform], reader, 0)
	dispatch(read_pipeline, uniforms)
	await get_tree().process_frame
	check(rd.buffer_get_data(vertex) == cleared, "Read-only binding lost synchronous data")
	var write_uniforms := rd.uniform_set_create([input_uniform, output_uniform], writer, 0)
	dispatch(write_pipeline, write_uniforms)
	var updated := false
	for frame in range(180):
		await get_tree().process_frame
		var bytes := rd.buffer_get_data(vertex)
		if bytes.size() == 16 and bytes.decode_u32(0) == 42:
			updated = true
			break
	check(updated, "GPU write returned stale CPU data")
	for rid in [
		uniforms,
		write_uniforms,
		read_pipeline,
		write_pipeline,
		reader,
		writer,
		vertex,
		copied,
		output
	]:
		rd.free_rid(rid)
	var result := "PASS" if failures == 0 else "FAIL (%d)" % failures
	print("[BufferCoherence] ", result)
	if OS.has_feature("web"):
		JavaScriptBridge.eval("window.bufferCoherenceResult = " + JSON.stringify(result))
