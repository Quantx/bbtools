@tool
class_name ResourceFormatLoaderBBEffectSequence extends ResourceFormatLoader

const extension := "efs"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var sequence := BBEffectSequence.new()
	sequence.resource_name = path.get_file().get_slice(".", 0)
	
	var frame_count := file.get_16()
	for f in frame_count:
		var frame := BBEffectSequence.Frame.new()
		frame.index = file.get_16()
		frame.type = file.get_8() as BBEffectSequence.FrameType
		if frame.type == BBEffectSequence.FrameType.Delay:
			frame.delay = file.get_float()
		
		sequence.frames.append(frame)
	
	return sequence

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBEffectSequence" if path.get_extension() == extension else ""
