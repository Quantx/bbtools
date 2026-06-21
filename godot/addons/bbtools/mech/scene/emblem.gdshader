shader_type spatial;

uniform sampler2D albedo_texture: source_color, repeat_disable;
uniform float alpha_scale = 0.0;
uniform float depth_offset = 0.001;

const float uv_scale = 512.0;

void fragment() {
	vec4 albedo_color = texture(albedo_texture, UV * uv_scale);
	ALBEDO = albedo_color.rgb;
	ALPHA = albedo_color.a * alpha_scale;
	
	// Ensure that the emblem is drawn on top of the mech
	DEPTH = FRAGCOORD.z + depth_offset;
}