shader_type spatial;
render_mode unshaded;

uniform sampler2D background_albedo: source_color, repeat_disable;

void fragment() {
	ALBEDO = texture(background_albedo, SCREEN_UV).rgb;
}