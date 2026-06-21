@tool
class_name ResourceFormatLoaderBBLinesDefs extends ResourceFormatLoader

const extension := "lines"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var linesdefs := BBLinesDefs.new()
	
	var entry_count := file.get_32()
	for entry_idx in entry_count:
		var line_count := file.get_32()
		var vertices := file.get_buffer(line_count * 16).to_vector2_array()
		linesdefs.defines.append(vertices)
	
	return linesdefs

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBLinesDefs" if path.get_extension() == extension else ""
