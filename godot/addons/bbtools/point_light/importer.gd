@tool
class_name EditorSceneFormatImporterBBPointLight extends EditorSceneFormatImporter

func _get_extensions() -> PackedStringArray:
	return ["point_light"]

func _get_import_flags() -> int:
	return IMPORT_SCENE

func _import_scene(path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	
	var point_light := BBPointLight.new()
	point_light.name = "PointLight"
	
	point_light.omni_attenuation = 2.0
	
	point_light.life = file.get_float()
	point_light.duration = file.get_float()
	
	point_light.light_color = Color(file.get_float(), file.get_float(), file.get_float())
	point_light.omni_range = file.get_float()
	
	point_light.end_color = Color(file.get_float(), file.get_float(), file.get_float())
	point_light.end_size = file.get_float()
	
	return point_light
