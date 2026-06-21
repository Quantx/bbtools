shader_type spatial;
render_mode cull_disabled, specular_schlick_ggx;

uniform float roughness : hint_range(0.0, 1.0) = 0.6;
uniform float specular : hint_range(0.0, 1.0) = 0.0;
uniform float anisotropy : hint_range(0.0, 1.0) = 0.0;
uniform float metallic : hint_range(0.0, 1.0) = 1.0;

uniform sampler2DArray bump_texture: source_color, repeat_enable, filter_linear_mipmap;
uniform sampler2D fresnel_texture: hint_normal, repeat_disable;

uniform vec4 water_color: source_color = vec4(1.0);

#include "clouds.gdshaderinc"

varying smooth vec3 world_position;

const float water_scale = 100.0;

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	ROUGHNESS = roughness;
	SPECULAR = specular;
	ANISOTROPY = anisotropy;
	METALLIC = metallic;
	
	// Construct Bump Matrix (This was normally done on the CPU, but we can do it here for free instead)
	float camera_angle = atan(CAMERA_DIRECTION_WORLD.z, CAMERA_DIRECTION_WORLD.x) - (PI * 0.5);
	float camera_sin = sin(camera_angle);
	float camera_cos = cos(camera_angle);
	
	mat2 bump_mat = mat2(0.7) * mat2(
		vec2(camera_cos, -camera_sin), // Col 0
		vec2(camera_sin,  camera_cos)  // Col 1
	);
	
	vec2 water_uv = UV * water_scale;
	float water_scroll = (TIME + rand(floor(water_uv))) * 20.0;
	
	// Compute which bump map frame to use
	uvec3 bump_size = uvec3(textureSize(bump_texture, 0));
	uint bump_idx = uint(water_scroll) % bump_size.z;
	
	vec2 bump_uv = bump_mat * (texture(bump_texture, vec3(water_uv, float(bump_idx))).xy * 2.0 - 1.0);
	bump_uv = bump_uv * 0.5 + 0.5; // Map to range [0, 1] centered on (0.5, 0.5)
	
	vec4 fresnel_norm = texture(fresnel_texture, bump_uv);
	
	NORMAL_MAP_DEPTH = 1.0;
	NORMAL_MAP = fresnel_norm.xyz;
	
	vec3 eye_dir = mat3(INV_VIEW_MATRIX) * -VIEW;
	vec3 sky_dir = reflect(eye_dir, vec3(0, 1, 0));
	
	vec4 sky_color = get_cloud_color(world_position, sky_dir, bump_uv);
	sky_color.rgb *= (1.0 - fresnel_norm.a);
	
	ALBEDO = mix(sky_color.rgb, water_color.rgb, fresnel_norm.a);
	ALPHA = water_color.a;
}