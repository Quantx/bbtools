shader_type spatial;

uniform sampler2D terrain : source_color, repeat_disable, filter_linear_mipmap_anisotropic;
uniform sampler2D ground : source_color;

uniform vec2 ground_scale = vec2(70.0);
uniform float ground_rotation_scale = 1.0;

mat2 rotation_mat(float angle) {
	return mat2(vec2(cos(angle), -sin(angle)), vec2(sin(angle), cos(angle)));
}

float rand(vec2 index) {
	return fract(sin(dot(index, vec2(12.9898, 78.233))) * 43758.5453);
}

void fragment() {
	vec2 gnd_uv = UV * ground_scale;
	mat2 gnd_trans = rotation_mat(ground_rotation_scale * rand(floor(gnd_uv)));
	
	vec4 gnd_col = texture(ground, gnd_trans * gnd_uv);
	vec4 terr_col = texture(terrain, UV);
	ALBEDO = mix(terr_col.rgb, gnd_col.rgb, 0.5);
}
