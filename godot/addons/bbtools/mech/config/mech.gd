class_name BBMechConfig extends Resource

enum WeightType {
	Light,
	Medium,
	Heavy,
}

enum ClassType {
	Standard,
	Support,
	Scout,
	Assult
}

@export var id: int

@export_enum("Gen 1", "Gen 2", "Gen 3", "Gen 2S", "Gen 1S", "Jar") var cockpit_type: int
@export_enum("Gen 1", "Gen 2", "Gen 3") var generation: int
@export var manufacturer: int
@export var weight_type: WeightType
@export var class_type: ClassType
@export var profile_description: int
@export var mounts: int
@export var ticket_cost: int

# This cannot be exported directly due to cyclical dependencies
@export_file("*.mech_scene") var scene_path: String
var scene: PackedScene

const paint_area_count: int = 11
@export var palettes: Array[ColorPalette]

#region engine
@export var rpm_min: float
@export var rpm_rate: float

@export var engine_rpms: PackedFloat32Array
@export var engine_torques: PackedFloat32Array

@export var override_rpm: float
@export var override_torque: float

@export var tier_r: float
@export var gears: PackedFloat32Array
@export var gear_f: float

@export var wheel_torque: float
@export var wheel_start_speed: float

@export var internal_resistance: float

@export var slope_gear_a: int # 16.0 <= slope < 26.0
@export var slope_gear_b: int # 26.0 <= sloep < 39.0

@export var weight: float
@export var brake: float

@export var drag_coefficient: float
@export var drag_size: Vector2

@export var turn_speed: float
@export var balancer: float

var max_speeds: PackedFloat32Array
var max_speeds_override: PackedFloat32Array
#endregion

@export var health_torso: int
@export var health_leg_r: int
@export var health_leg_l: int
@export var health_opt_armor: int

@export var resistance_front: float
@export var resistance_side: float
@export var resistance_rear: float

@export var tank_capacity_main: float = 24000.0
@export var tank_capacity_sub: float = 10000.0

#region loadout
@export var loadout_weight_standard: int
@export var loadout_weight_max: int

@export var loadout_mweps_configs: Array[BBWeaponConfig]
@export var loadout_mweps_presets: PackedInt32Array
@export var loadout_mweps_fixed: PackedByteArray

@export var loadout_sweps_configs: Array[BBWeaponConfig]
@export var loadout_sweps_presets: PackedInt32Array # Indices into swep arrays
@export var loadout_sweps_fixed: PackedByteArray

@export var tank_count_max: int
@export var tank_count_preset: int
#endregion

func _init() -> void:
	_init_deferred.call_deferred()

func _init_deferred() -> void:
	scene = load(scene_path) as PackedScene
	
	# Compute max speeds for each gear
	var gear_coeff := tier_r * 0.05236 / gear_f
	for gear in gears:
		var gear_ratio := gear_coeff / gear
		max_speeds.append(engine_rpms[-1] * gear_ratio)
		max_speeds_override.append(override_rpm * gear_ratio)

func get_gear(gear: int) -> float:
	return gears[maxi(gear, 0)]

func rpm_to_torque(rpm: float) -> float:
	for i in range(1, engine_rpms.size()):
		if rpm < engine_rpms[i]:
			return remap(rpm, engine_rpms[i - 1], engine_rpms[i], engine_torques[i - 1], engine_torques[i])
	return engine_torques[-1]

func speed_to_gear(speed: float, override: bool) -> int:
	if max_speeds.is_empty() || max_speeds_override.is_empty():
		return 0
	
	if speed >= wheel_start_speed:
		return 5
	
	for i in range(1, 5):
		var max_speed := max_speeds_override[i] if override else max_speeds[i]
		if speed < max_speed * 0.8:
			return i
	
	return 4

func change_gear_ratio(gear_new: int, gear_old: int) -> float:
	if gear_new == gear_old:
		return 1.0
	
	if gear_old == 0:
		return get_gear(gear_new) * 0.05
	
	if gear_new != 0:
		return get_gear(gear_new) / get_gear(gear_old)
	
	return 1.0

func tank_capacity(tank_idx: int) -> float:
	return tank_capacity_main if tank_idx == 0 else tank_capacity_sub

func has_opt_armor() -> bool:
	return health_opt_armor > 0
