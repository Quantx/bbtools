shader_type sky;

#include "clouds.gdshaderinc"

const float sun_size = 0.5;

uniform vec4 fog_color : source_color;

void sky() {
	if (!AT_CUBEMAP_PASS) {
		FOG = fog_color;
		if (EYEDIR.y > 0.0) {
			vec4 sky_color = get_cloud_color(POSITION, EYEDIR, vec2(0.0));
			COLOR = sky_color.rgb;
			FOG.a = sky_color.a;
		}
		
		// Handle Sun
		if (LIGHT0_ENABLED) {
			float eye_angle = acos(dot(LIGHT0_DIRECTION, EYEDIR));
			float sun_angle = sun_size * LIGHT0_ENERGY;
			COLOR = mix(COLOR, LIGHT0_COLOR, clamp(1.0 - eye_angle / sun_angle, 0.0, 1.0));
		}
	}
}