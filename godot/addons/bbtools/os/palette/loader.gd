@tool
class_name ResourceFormatLoaderBBPalette extends ResourceFormatLoader

const extension := "palette"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var palette := ColorPalette.new()
	
	var color_count := file.get_32()
	palette.colors = file.get_buffer(color_count * 16).to_color_array()
	
	return palette

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "ColorPalette" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return ""
