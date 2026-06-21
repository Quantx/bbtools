@tool
class_name ResourceFormatLoaderBBStage extends ResourceFormatLoader

const extension := "stage"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var stage := BBStage.new()
	
	stage.draw_shadows = file.get_8() as bool
	stage.draw_rain = file.get_8() as bool
	
	stage.tactics_time = file.get_32()
	stage.tickets_a = file.get_32()
	stage.tickets_b = file.get_32()
	
	stage.world_light.r = file.get_float()
	stage.world_light.g = file.get_float()
	stage.world_light.b = file.get_float()
	stage.world_light.a = file.get_float()
	
	stage.world_specular.r = file.get_float()
	stage.world_specular.g = file.get_float()
	stage.world_specular.b = file.get_float()
	stage.world_specular.a = file.get_float()
	
	stage.world_ambient.r = file.get_float()
	stage.world_ambient.g = file.get_float()
	stage.world_ambient.b = file.get_float()
	stage.world_ambient.a = file.get_float()
	
	stage.fog_color.r = file.get_float()
	stage.fog_color.g = file.get_float()
	stage.fog_color.b = file.get_float()
	stage.fog_color.a = file.get_float()
	
	stage.fog_start = file.get_float()
	stage.fog_end = file.get_float()
	
	stage.sky_fog_start = file.get_float()
	stage.sky_fog_end = file.get_float()
	
	stage.sun_color.r = file.get_float()
	stage.sun_color.g = file.get_float()
	stage.sun_color.b = file.get_float()
	stage.sun_color.a = file.get_float()
	
	stage.sun_flash_power = file.get_float()
	stage.sun_back_size = file.get_float()
	stage.sun_front_size = file.get_float()
	
	stage.shadow_start = file.get_float()
	stage.shadow_end = file.get_float()
	stage.shadow_yaw = file.get_float()
	stage.shadow_pitch = file.get_float()
	
	stage.sky_height = file.get_float()
	stage.sky_velocity_0.x = file.get_float()
	stage.sky_velocity_0.y = file.get_float()
	stage.sky_velocity_1.x = file.get_float()
	stage.sky_velocity_1.y = file.get_float()

	stage.water_color.r = file.get_float()
	stage.water_color.g = file.get_float()
	stage.water_color.b = file.get_float()
	stage.water_color.a = file.get_float()
	
	return stage

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBStage" if path.get_extension() == extension else ""
