@tool
class_name ResourceFormatLoaderBBSpriteSheet extends ResourceFormatLoader

const extension := "spritesheet"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var texture_path := file.get_pascal_string()
	
	var sprite := BBSpriteSheet.new()
	sprite.resource_name = path.get_file().get_slice(".", 0)
	
	sprite.texture = load(texture_path) as Texture
	sprite.offset.x = file.get_float()
	sprite.offset.y = file.get_float()
	
	var frame_count := file.get_16()
	for _f in frame_count:
		var start: Vector2
		start.x = file.get_float()
		start.y = file.get_float()
		
		var end: Vector2
		end.x = file.get_float()
		end.y = file.get_float()
		
		var scale: Vector2
		scale.x = file.get_float()
		scale.y = file.get_float()
		
		sprite.frames.append(BBSpriteSheet.Frame.new(start, end, scale))
	
	return sprite

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBSpriteSheet" if path.get_extension() == extension else ""
