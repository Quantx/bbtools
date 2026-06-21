shader_type spatial;
render_mode cull_disabled, blend_add;

uniform sampler2D albedo_texture : source_color, repeat_enable;
uniform float albedo_texture_repeat = 1.0;
uniform sampler2D albedo_gradient : source_color, repeat_disable;

void fragment() {
	vec2 albedo_texture_uv = UV * vec2(1.0, albedo_texture_repeat);
	vec4 albedo_texture_color = texture(albedo_texture, albedo_texture_uv);
	vec4 albedo_gradient_color = texture(albedo_gradient, UV.yy);
	
	vec4 color = albedo_texture_color * albedo_gradient_color;
	
	ALBEDO = color.rgb;
	ALPHA = color.a;
	EMISSION = color.rgb;
	
	RIM = 1.0; // This ensures that the back faces receive the same lighting as the front
}