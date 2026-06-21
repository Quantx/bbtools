render_mode unshaded, cull_disabled, skip_vertex_transform;

const uint IS_BILLBOARD = 0u;
const uint USE_VERTEX_COLOR_ALPHA = 1u;
const uint NO_FOG = 2u;
const uint IS_COCKPIT = 3u;

uniform sampler2D albedo_texture : source_color, repeat_disable;

varying flat vec4 CUSTOM_DATA;

bool is_set(uint bits, uint index) {
	uint mask = 1u << index;
	return (bits & mask) == mask;
}

void vertex() {
	CUSTOM_DATA = INSTANCE_CUSTOM;
	
	vec2 uv_data = CUSTOM_DATA.xy;
	vec2 uv_offset = floor(uv_data);
	vec2 uv_scale = uv_data - uv_offset;
	
	UV *= uv_scale;
	UV += uv_offset / vec2(textureSize(albedo_texture, 0));
	
	uint flags = floatBitsToUint(CUSTOM_DATA.z);
	
	if (is_set(flags, IS_BILLBOARD)) {
		// Billboard Mode: Enabled
		MODELVIEW_MATRIX = VIEW_MATRIX * mat4(
			MAIN_CAM_INV_VIEW_MATRIX[0],
			MAIN_CAM_INV_VIEW_MATRIX[1],
			MAIN_CAM_INV_VIEW_MATRIX[2],
			MODEL_MATRIX[3]);

		// Billboard Keep Scale: Enabled
		MODELVIEW_MATRIX = MODELVIEW_MATRIX * mat4(
			vec4(length(MODEL_MATRIX[0].xyz), 0.0, 0.0, 0.0),
			vec4(0.0, length(MODEL_MATRIX[1].xyz), 0.0, 0.0),
			vec4(0.0, 0.0, length(MODEL_MATRIX[2].xyz), 0.0),
			vec4(0.0, 0.0, 0.0, 1.0));

		MODELVIEW_NORMAL_MATRIX = mat3(MODELVIEW_MATRIX);
	}

	VERTEX = (MODELVIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
	NORMAL = normalize((MODELVIEW_MATRIX * vec4(NORMAL, 0.0)).xyz);
	BINORMAL = normalize((MODELVIEW_MATRIX * vec4(BINORMAL, 0.0)).xyz);
	TANGENT = normalize((MODELVIEW_MATRIX * vec4(TANGENT, 0.0)).xyz);
}

void fragment() {
	uint flags = floatBitsToUint(CUSTOM_DATA.z);
	
	vec4 albedo_color = texture(albedo_texture, UV);
	
	vec4 out_color = albedo_color * COLOR;
	if (is_set(flags, USE_VERTEX_COLOR_ALPHA)) {
		// Add/Sub do not use vertex alpha
		out_color.a = albedo_color.a;
	}
	
	//if (is_set(flags, IS_COCKPIT)) {
		//// Ensure that effects are rendered on top of the cockpit
		//DEPTH = FRAGCOORD.z + 0.002;
	//}
	
	ALBEDO = out_color.rgb;
	ALPHA = out_color.a;
}