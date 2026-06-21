class_name BBBoot extends Resource

@export var font: Font
@export var strings_path: String# = "res://proprietary/loc/os/boot/strings.txt"

@export var texture: Texture2D
@export var spritesheet: BBSpriteSheet

@export var linesdefs: BBLinesDefs

@export var duration: float
@export var draws: Array[BBBootDrawBase]

func get_spritesheet_region(idx: int) -> Rect2:
	return spritesheet.frames[idx].image_region(texture.get_size())

func get_strings() -> PackedStringArray:
	var strings_file := FileAccess.open(strings_path, FileAccess.READ)
	var strings := strings_file.get_as_text().split("\n")
	for i in strings.size():
		strings[i] = strings[i].strip_edges()
	
	return strings
