@tool
class_name ResourceFormatLoaderBBEffect extends ResourceFormatLoader

const extension := "efg"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var eff_grp := BBEffectConfigGroup.new()
	eff_grp.resource_name = path.get_file().get_slice(".", 0)
	
	var eff_count := file.get_32()
	for eff_idx in eff_count:
		var eff := BBEffectConfig.new()
		eff.resource_name = "%s_%02d" % [eff_grp.resource_name, eff_idx]
		
		eff.type = file.get_8() as BBEffectConfig.EffectType
		if eff.is_2D():
			var spritesheet_path := file.get_pascal_string()
			eff.spritesheet = load(spritesheet_path) as BBSpriteSheet
			
			var sequence_path := file.get_pascal_string()
			eff.sequence = load(sequence_path) as BBEffectSequence
		
		if eff.is_3D():
			eff.model_path = file.get_pascal_string()
		
		eff.blend = file.get_8() as BBEffectConfig.BlendType
		
		eff.life = file.get_float()
		eff.delay = file.get_float()
		
		eff.priority = file.get_16()
		eff.flags = file.get_16()
		
		var child_eff_path := file.get_pascal_string()
		if !child_eff_path.is_empty():
			eff.child_effect = load(child_eff_path) as BBEffectConfigGroup
			eff.child_effect_interval = file.get_float()
		
		eff.vertex_color = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
		eff.damping_color = Color(file.get_float(), file.get_float(), file.get_float(), file.get_float())
		eff.damping_delay = file.get_float()
		
		eff.initial = BBEffectConfig.EffectTransform.new(file)
		eff.velocity = BBEffectConfig.EffectTransform.new(file)
		eff.acceleration = BBEffectConfig.EffectTransform.new(file)
		
		eff.gravity_acceleration.x = file.get_float()
		eff.gravity_acceleration.y = file.get_float()
		eff.gravity_acceleration.z = file.get_float()
		
		if eff.is_repeat():
			eff.repeat_count = file.get_16()
			eff.repeat_count_random = file.get_16()
			eff.repeat_interval = file.get_float()
			
			eff.vertex_color_random = Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
			
			eff.life_random = file.get_float()
			
			eff.initial_position_random.x = file.get_float()
			eff.initial_position_random.y = file.get_float()
			eff.initial_position_random.z = file.get_float()
			
			eff.initial_scale_xy_random = file.get_float()
			
			eff.initial_rotation_z_random = file.get_float()
			
			eff.velocity_position_rotation_random.x = file.get_float()
			eff.velocity_position_rotation_random.y = file.get_float()
			eff.velocity_position_rotation_random.z = file.get_float()
			
			eff.velocity_position_offset_random.x = file.get_float()
			eff.velocity_position_offset_random.y = file.get_float()
			eff.velocity_position_offset_random.z = file.get_float()
		
		eff_grp.effect_configs.append(eff)
	
	return eff_grp

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBEffectConfigGroup" if path.get_extension() == extension else ""
