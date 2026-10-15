shader_type spatial;

uniform sampler2D grass_texture : source_color;

void fragment() {
	vec2 grass_uv = vec2(0.01, 0.51) + fract(UV * 1.6) * 0.48;
	vec3 grass = texture(grass_texture, grass_uv).rgb;
	// The atlas grass tile is red; swap red and green for the preview.
	ALBEDO = grass.grb;
	ROUGHNESS = 1.0;
}
