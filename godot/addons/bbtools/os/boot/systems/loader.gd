@tool
class_name ResourceFormatLoaderBBBootSystems extends ResourceFormatLoader

const extension := "boot_systems"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var systems := BBBootSystems.new()
	
	var system_count := file.get_32()
	
	systems.system_count = system_count
	
	systems.switch_error_color = Color(file.get_8(), file.get_8(), file.get_8(), file.get_8())
	systems.switch_primary_color = Color(file.get_8(), file.get_8(), file.get_8(), file.get_8())
	
	systems.switch_error_vertices = file.get_buffer(system_count * 16).to_vector2_array()
	
	var switch_progress_quad_count := file.get_32()
	systems.switch_progress_quad_count = switch_progress_quad_count
	systems.switch_progress_vertices = file.get_buffer(system_count * switch_progress_quad_count * 32).to_vector2_array()
	
	systems.switch_success_text = file.get_pascal_string()
	systems.switch_success_positions = file.get_buffer(system_count * 8).to_vector2_array()
	
	var startup_progress_count := file.get_32()
	systems.startup_progress_count = startup_progress_count
	systems.startup_progress_positions = file.get_buffer(system_count * startup_progress_count * 8).to_vector2_array()
	
	return systems

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBBootSystems" if path.get_extension() == extension else ""
