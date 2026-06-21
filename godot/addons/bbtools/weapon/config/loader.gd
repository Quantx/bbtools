@tool
class_name ResourceFormatLoaderBBWeaponConfig extends ResourceFormatLoader

const extension := "weapon"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var config := BBWeaponConfig.new()
	config.id = file.get_8()
	config.type = file.get_8()
	
	var prefix := BBTools.get_content_prefix(path)
	config.resource_name = BBWeaponConfig.config_name(prefix, config.type, config.id)
	
	config.category = file.get_8()
	config.damage_type = file.get_8()
	config.tracking = file.get_8()
	config.impact_type = file.get_8()
	
	var type_name := BBWeaponConfig.weapon_type_strings[config.type]
	config.weapon_scene_path = base_path.path_join("%s_%02d.weapon_scene" % [type_name.to_upper(),config.id])
	config.projectile_scene_path = base_path.path_join("%s_%02d.projectile_scene" % [type_name.to_upper(), config.id])
	
	config.display_name = file.get_pascal_string()
	
	config.torso_turn_rate = file.get_float()
	config.weight = file.get_8()
	
	config.muzzle_count.x = file.get_8()
	config.muzzle_count.y = file.get_8()
	
	config.muzzle_offset.x = file.get_float()
	config.muzzle_offset.y = file.get_float()
	
	config.firing_interval = file.get_float()
	config.reload_interval = file.get_float()
	
	config.rapid_fire = file.get_16()
	
	config.projectile_count = file.get_16()
	config.magazine_count = file.get_8()
	
	config.volley_count = file.get_8()
	config.volley_interval = file.get_float()
	config.volley_spread = file.get_float()
	
	config.recoil = file.get_float()
	config.mech_recoil = file.get_8() as bool
	config.charge_delay = file.get_float()
	
	config.initial_velocity = file.get_float()
	config.boost_max = file.get_float()
	config.boost_rate = file.get_float()
	config.gravity_acceleration = file.get_float()
	
	config.range_min = file.get_float()
	config.range_max = file.get_float()
	
	config.damage_range = file.get_float()
	config.damage_min = file.get_16()
	config.damage_max = file.get_16()
	
	config.fire_probability = file.get_8()
	config.fire_damage = file.get_8()
	
	return config

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBWeaponConfig" if path.get_extension() == extension else ""
