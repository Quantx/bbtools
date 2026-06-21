uniform int sky_mode = 0; // 0 = Dual Flat, 1 = Spherical
uniform float sky_height;

uniform sampler2D sky0_texture : source_color, repeat_enable;
uniform vec2 sky0_velocity;

uniform sampler2D sky1_texture : source_color, repeat_enable;
uniform vec2 sky1_velocity;

uniform float fog_start;
uniform float fog_end;

uniform vec2 sky_scale = vec2(5000.0);

mat2 rotation_mat(float angle) {
	return mat2(vec2(cos(angle), -sin(angle)), vec2(sin(angle), cos(angle)));
}

float rand(vec2 index) {
	return fract(sin(dot(index, vec2(12.9898, 78.233))) * 43758.5453);
}

vec4 _cloud_dual_flat(vec3 pos, vec3 dir, vec2 distort_uv) {
	float sky_dist = sky_height - pos.y;
	
	if (sky_dist <= 0.0) {
		return vec4(1.0);
	}
	
	vec3 sky_ray = dir * abs(sky_dist / dir.y);
	float sky_ray_len = length(sky_ray.xz);
	vec2 sky_uv = (sky_ray + pos).xz / sky_scale;
	
	sky_uv += distort_uv;
	
	vec2 sky_uv0 = sky_uv + sky0_velocity * TIME;
	vec2 sky_uv1 = sky_uv + sky1_velocity * TIME;
	
	sky_uv0 *= 1.0;
	sky_uv1 *= 1.5;
	
	mat2 sky_trans0 = rotation_mat(rand(floor(sky_uv0)));
	mat2 sky_trans1 = rotation_mat(rand(floor(sky_uv1)));
	
	vec4 sky0_color = texture(sky0_texture, sky_uv0);
	vec4 sky1_color = texture(sky1_texture, sky_uv1);
	float fog_ratio = smoothstep(fog_start, fog_end, sky_ray_len);
	
	vec4 sky_color = mix(sky0_color, sky1_color, 0.5);
	sky_color.a = fog_ratio;
	return sky_color;
}

vec4 _cloud_spherical(vec3 pos, vec3 dir, vec2 distort_uv) {
	float theta = acos(abs(dir.y));
	float phi = atan(dir.z, dir.x) + PI;
	
	vec2 uv = vec2(phi / TAU, theta / PI);
	
	vec4 sky0_color = texture(sky0_texture, uv);
	
	return vec4(sky0_color.rgb, 0.0);
}

vec4 get_cloud_color(vec3 pos, vec3 dir, vec2 distort_uv) {
	if (sky_mode == 0) {
		return _cloud_dual_flat(pos, dir, distort_uv);
	} else if (sky_mode == 1) {
		return _cloud_spherical(pos, dir, distort_uv);
	}
	
	return vec4(0.0);
}