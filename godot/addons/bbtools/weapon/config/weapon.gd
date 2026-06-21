class_name BBWeaponConfig extends Resource

enum WeaponType {
	MWEP,
	SWEP,
	CWEP,
	EWEP
}

const weapon_type_strings: Dictionary[WeaponType,StringName] = {
	WeaponType.MWEP: &"mwep",
	WeaponType.SWEP: &"swep",
	WeaponType.CWEP: &"cwep",
	WeaponType.EWEP: &"ewep",
}

static func config_name(prefix: StringName, wep_type: WeaponType, id: int) -> StringName:
	var wep_str := weapon_type_strings[wep_type]
	return &"%s:%s:%02d" % [prefix, wep_str, id]

static func config_path(name: StringName) -> String:
	var parts := name.split(":")
	if parts.size() != 3:
		push_error("Invalid config name: %s" % name)
		return ""
	
	var base_path := BBTools.get_content_path(parts[0])
	return base_path.path_join("weapons/%s/%s.weapon" % [parts[1], parts[2]])

enum WeaponCategory {
	Artillery = 0,
	Laser = 1,
	Bullet = 2,
	Missile = 3,
	Mortar = 4,
	Melee = 5,
	Mines = 6,
	Flamethrower = 7,
	CruiseMissile = 8, # Bigass missile`
	Cosmetic = 9, # Used by cosmetic weapons plus the MM-drop weapon
	MLRS = 10,
	# 11, 12 are unused
	Spear = 13,
	Gauss = 14,
}

enum DamageType {
	Standard = 0,
	# 1 is unused
	Proximity = 2,
	Mines = 3,
	Artillery = 4,
	Melee = 5,
	Flamethrower = 6,
	MLRS = 7,
}

enum ImpactType {
	None = 0, # Used by weapons which need no effect
	Bullet1 = 1,
	Bullet2 = 2,
	Laser1 = 3, # Used by MWEP MGs
	Laser2 = 4,
	Laser3 = 5,
	Laser4 = 6,
	MissileBig = 7,
	Missile1 = 8,
	Missile2 = 9,
	Artillery1 = 10,
	Artillery2 = 11,
	Artillery3 = 12,
	Napalm = 13,
	Flamethrower = 14,
	Melee = 15,
	Plasma = 16,
	Stun = 17,
	Mine = 18,
	Marker = 19,
	# 20, 21, 22 are unused
	Grenade = 23,
}

const mlrs_hurtbox_height : float = 2000.0

@export var id: int
@export var type: WeaponType
@export var category: WeaponCategory
@export var damage_type: DamageType
@export var tracking: int
@export var impact_type: ImpactType

@export_file("*.weapon_scene") var weapon_scene_path: String
@export_file("*.projectile_scene") var projectile_scene_path: String

# These cannot be exported directly due to cyclical dependencies
var weapon_scene: PackedScene:
	get = _get_weapon_scene
var projectile_scene: PackedScene:
	get = _get_projectile_scene

@export var display_name: String

@export var torso_turn_rate: float
@export var weight: int

@export var muzzle_count := Vector2i.ONE
@export var muzzle_offset: Vector2

@export var firing_interval: float
@export var reload_interval: float

@export var rapid_fire: int

@export var projectile_count: int
@export var magazine_count: int

@export var volley_count: int
@export var volley_interval: float
@export var volley_spread: float

@export var recoil: float
@export var mech_recoil: bool
@export var charge_delay: float

#region projectile
@export var initial_velocity: float
@export var boost_max: float
@export var boost_rate: float
@export var gravity_acceleration: float

@export var range_min: float
@export var range_max: float

@export var damage_range: float
@export var damage_min: int
@export var damage_max: int

@export var fire_probability: int
@export var fire_damage: int
#endregion

func _get_weapon_scene() -> PackedScene:
	if !weapon_scene:
		weapon_scene = load(weapon_scene_path) as PackedScene
	return weapon_scene

func _get_projectile_scene() -> PackedScene:
	if !projectile_scene:
		projectile_scene = load(projectile_scene_path) as PackedScene
	return projectile_scene

func get_aoe_damage(distance: float) -> int:
	return maxi(roundi(lerpf(damage_max, damage_min, abs(distance) / damage_range)), damage_min)

func distance_squared_to(from: Vector3, to: Vector3) -> float:
	if is_artillery():
		# Artillery doesn't factor in the vertical component
		return Vector2(from.x, from.z).distance_squared_to(Vector2(to.x, to.z))
	
	return from.distance_squared_to(to)

func distance_to(from: Vector3, to: Vector3) -> float:
	return sqrt(distance_squared_to(from, to))

func is_artillery() -> bool:
	return category == WeaponCategory.Artillery || category == WeaponCategory.Mortar

func is_MLRS() -> bool:
	return category == WeaponCategory.MLRS

func is_melee() -> bool:
	return category == WeaponCategory.Melee

func ignore_resistance() -> bool:
	return impact_type > ImpactType.Laser4 && impact_type != ImpactType.Marker

func causes_impact() -> bool:
	return damage_type == DamageType.Standard || damage_type == DamageType.Melee

func random_ignite() -> bool:
	return randi() % 100 < fire_probability

func get_muzzle_offset(muzzle_idx: int) -> Vector3:
	muzzle_idx = posmod(muzzle_idx, muzzle_count.x * muzzle_count.y)
	var muzzle_offset_2d := muzzle_offset * Vector2(muzzle_idx % muzzle_count.x, muzzle_idx / muzzle_count.x)
	return Vector3(muzzle_offset_2d.x, muzzle_offset_2d.y, 0.0)

func set_mlrs_bomb_warn(collider: CollisionShape3D, muzzle_dir: Vector3) -> void:
	assert(collider.shape is BoxShape3D)
	
	var muzzle_angle := muzzle_dir.angle_to(Vector3(muzzle_dir.x, 0.0, muzzle_dir.z))
	#print(muzzle_dir, rad_to_deg(muzzle_angle))
	
	var warn_size_inner := Vector3(0.0, mlrs_hurtbox_height, range_max - range_min)
	var warn_size_outer := warn_size_inner + Vector3.ONE * (damage_range * 2.0)
	warn_size_inner.z *= cos(muzzle_angle)
	warn_size_outer.z *= cos(muzzle_angle)
	
	var box_shape := collider.shape as BoxShape3D
	box_shape.size = warn_size_outer
	collider.position = Vector3(0.0, -warn_size_inner.y * 0.5, warn_size_inner.z * 0.5)
