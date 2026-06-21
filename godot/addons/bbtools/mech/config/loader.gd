@tool
class_name ResourceFormatLoaderBBMechConfig extends ResourceFormatLoader

const extension := "mech"

func _load(path: String, _original_path: String, _use_sub_threads: bool, _cache_mode: int) -> Variant:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	if !file:
		print("Failed to open file: %s, got error: %s" % [path, error_string(FileAccess.get_open_error())])
		return null
	
	var config := BBMechConfig.new()
	config.id = file.get_8()
	config.resource_name = "loc:mech_%02d" % config.id
	
	config.scene_path = base_path.path_join("Mech_%02d.mech_scene" % config.id)
	
	config.cockpit_type = file.get_8()
	config.generation = file.get_8()
	config.manufacturer = file.get_8()
	config.weight_type = file.get_8()
	config.class_type = file.get_8()
	config.profile_description = file.get_8()
	config.mounts = file.get_8()
	config.ticket_cost = file.get_8()
	
	for i in 4:
		var palette_path := base_path.path_join("%d.palette" % i)
		var palette := load(palette_path) as ColorPalette
		assert(palette.colors.size() == BBMechConfig.paint_area_count)
		config.palettes.append(palette)
	
	config.rpm_min = file.get_float()
	config.rpm_rate = file.get_float()
	
	for i in 4:
		config.engine_rpms.append(file.get_float())
		config.engine_torques.append(file.get_float())
	
	config.override_rpm = file.get_float()
	config.override_torque = file.get_float()
	
	config.tier_r = file.get_float()
	for i in 6:
		config.gears.append(file.get_float())
	config.gear_f = file.get_float()
	
	config.wheel_torque = file.get_float()
	config.wheel_start_speed = file.get_float()
	
	config.internal_resistance = file.get_float()
	
	config.slope_gear_a = file.get_8()
	config.slope_gear_b = file.get_8()
	
	config.weight = file.get_float()
	config.brake = file.get_float()
	
	config.drag_coefficient = file.get_float()
	config.drag_size.x = file.get_float()
	config.drag_size.y = file.get_float()
	
	config.turn_speed = file.get_float()
	config.balancer = file.get_float()
	
	config.health_torso = file.get_16()
	config.health_leg_r = file.get_16()
	config.health_leg_l = file.get_16()
	config.health_opt_armor = file.get_16()
	
	config.resistance_front = file.get_float()
	config.resistance_side = file.get_float()
	config.resistance_rear = file.get_float()
	
	# Loadout
	config.tank_capacity_main = file.get_float()
	config.tank_capacity_sub = file.get_float()
	
	config.loadout_weight_max = file.get_8()
	config.loadout_weight_standard = file.get_8()
	
	var mwep_count := file.get_8()
	for i in mwep_count:
		var mwep_path := file.get_pascal_string()
		var mwep_config := load(mwep_path) as BBWeaponConfig
		config.loadout_mweps_configs.append(mwep_config)
	
	var mwep_presets := file.get_buffer(3)
	for mwep_idx in mwep_presets:
		config.loadout_mweps_presets.append(mwep_idx if mwep_idx != 0xFF else -1)
	
	config.loadout_mweps_fixed = file.get_buffer(3)
	
	var swep_count := file.get_8()
	for i in swep_count:
		var swep_path := file.get_pascal_string()
		var swep_config := load(swep_path) as BBWeaponConfig
		config.loadout_sweps_configs.append(swep_config)
	
	var swep_presets := file.get_buffer(3)
	for swep_idx in swep_presets:
		config.loadout_sweps_presets.append(swep_idx if swep_idx != 0xFF else -1)
	
	config.loadout_sweps_fixed = file.get_buffer(3)
	
	config.tank_count_max = file.get_8()
	config.tank_count_preset = file.get_8()
	
	return config

func _get_recognized_extensions() -> PackedStringArray:
	return [extension]

func _handles_type(type: StringName) -> bool:
	return ClassDB.is_parent_class(type, "Resource");

func _get_resource_type(path: String) -> String:
	return "Resource" if path.get_extension() == extension else ""

func _get_resource_script_class(path: String) -> String:
	return "BBMechConfig" if path.get_extension() == extension else ""
