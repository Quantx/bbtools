shader_type spatial;

uniform sampler2D albedo_texture : source_color, repeat_disable;
uniform float alpha_scissor_threshold : hint_range(0.0, 1.0, 0.01) = 0.1;
uniform float irradiance_scale : hint_range(0.0, 1.0, 0.01) = 0.1;

void fragment() {
	vec4 albedo = texture(albedo_texture, UV);
	ALBEDO = albedo.rgb;
	ALPHA = albedo.a;
	
	ALPHA_SCISSOR_THRESHOLD = alpha_scissor_threshold;
}