@tool
class_name ResourceFormatLoaderBBBoot extends ResourceFormatLoader

const extension := "boot_anim"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var boot := BBBoot.new()
	
	var font_path := file.get_pascal_string()
	boot.font = load(font_path) as Font
	
	boot.strings_path = file.get_pascal_string()
	
	var texture_path := file.get_pascal_string()
	boot.texture = load(texture_path) as Texture2D
	
	var spritesheet_path := file.get_pascal_string()
	boot.spritesheet = load(spritesheet_path) as BBSpriteSheet
	
	var lines_path := file.get_pascal_string()
	boot.linesdefs = load(lines_path) as BBLinesDefs
	
	boot.duration = file.get_float()
	
	var draw_count := file.get_32()
	for draw_idx in draw_count:
		var draw_type := file.get_8()
		var draw: BBBootDrawBase
		match draw_type:
			0:
				draw = BBBootDrawText.new()
				draw.string_idx = file.get_8()
				draw.string_length = file.get_32()
				
				draw.position.x = file.get_float()
				draw.position.y = file.get_float()
				
				draw.color = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
			1:
				draw = BBBootDrawQuad.new()
				draw.vertices = file.get_buffer(32).to_vector2_array()
				for i in 4:
					draw.colors.append(Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8()))
			2:
				draw = BBBootDrawLine.new()
				draw.start.x = file.get_float()
				draw.start.y = file.get_float()
				draw.end.x = file.get_float()
				draw.end.y = file.get_float()
				
				draw.color = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
			3:
				draw = BBBootDrawSpriteDef.new()
				draw.sprite_idx = file.get_8()
				
				draw.start.x = file.get_float()
				draw.start.y = file.get_float()
				draw.end.x = file.get_float()
				draw.end.y = file.get_float()
				
				draw.start2.x = file.get_float()
				draw.start2.y = file.get_float()
				draw.end2.x = file.get_float()
				draw.end2.y = file.get_float()
				
				draw.color = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
			4:
				draw = BBBootDrawLinesDef.new()
				draw.lines_idx = file.get_8()
				
				draw.position.x = file.get_float()
				draw.position.y = file.get_float()
				draw.rotation = file.get_float()
				draw.scale = file.get_float()
				
				draw.color = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
		
		var anim_count := file.get_32()
		for anim_idx in anim_count:
			var time := file.get_float()
			var duration := file.get_float()
			
			var anim_type := file.get_8()
			var anim: BBBootAnimBase
			match anim_type:
				0:
					anim = BBBootAnimStart.new()
				1:
					anim = BBBootAnimPoints.new()
					anim.vertices = file.get_buffer(32).to_vector2_array()
				2:
					anim = BBBootAnimRotate.new()
					anim.clockwise = file.get_8() as bool
				3:
					anim = BBBootAnimColor.new()
					anim.color = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
				4:
					anim = BBBootAnimColors.new()
					for i in 4:
						anim.colors.append(Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8()))
				5:
					anim = BBBootAnimText.new()
				6:
					anim = BBBootAnimScale.new()
					anim.scale = file.get_float()
				7:
					anim = BBBootAnimStop.new()
			
			anim.time = time
			anim.duration = duration
			
			draw.animations.append(anim)
		
		boot.draws.append(draw)
	
	return boot

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBBoot" if path.get_extension() == extension else ""
