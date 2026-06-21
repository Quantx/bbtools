@tool
class_name ResourceFormatLoaderBBSpriteDefs extends ResourceFormatLoader

const extension := "sprites"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var spritedefs := BBSpriteDefs.new()
	
	var sprite_count := file.get_32()
	for sprite_idx in sprite_count:
		var sprite := BBSprite.new()
		
		var spritesheet_path := file.get_pascal_string()
		sprite.spritesheet = load(spritesheet_path) as BBSpriteSheet
		
		sprite.frame_idx = file.get_32()
		
		sprite.position.x = file.get_float()
		sprite.position.y = file.get_float()
		sprite.origin.x = file.get_float()
		sprite.origin.y = file.get_float()
		sprite.size.x = file.get_float()
		sprite.size.y = file.get_float()
		sprite.rotation = file.get_float()
		sprite.scale = file.get_float()
		
		var use_color := file.get_8() as bool
		if use_color:
			sprite.color = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
		else:
			sprite.pallete_idx = file.get_32()
		
		spritedefs.defines.append(sprite)
	
	return spritedefs

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBSpriteDefs" if path.get_extension() == extension else ""
