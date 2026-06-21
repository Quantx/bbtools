shader_type spatial;

const float S16_MAX = 32768.0;

uniform sampler3D albedo_texture: source_color, repeat_disable;
uniform float albedo_uv_z = 0.0;

uniform vec4 override_color: source_color = vec4(0);

void fragment() {
	vec3 albedo_texture_size = vec3(textureSize(albedo_texture, 0));
	vec3 albedo_uv = vec3(UV * S16_MAX, albedo_uv_z) / albedo_texture_size;
	vec4 albedo = texture(albedo_texture, albedo_uv);
	
	ALPHA_SCISSOR_THRESHOLD = 0.5;
	
	ALBEDO = mix(albedo.rgb, override_color.rgb, override_color.a);
	ALPHA = albedo.a;
}