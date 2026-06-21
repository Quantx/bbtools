class_name BBWeapon extends Node3D

enum Flags {
	Shoulder	= 0x0001_0000, # Weapon does not go in the SWEP BOX
	AttackAnim	= 0x0002_0000, # Weapon plays an attack animation
	EquipAnim	= 0x0004_0000, # Weapon plays an equip animation
	Melee		= 0x0008_0000, # Melee weapon (not including shields)
	
	Shield2		= 0x0020_0000, # Only used by Jar Shield
	Gauss		= 0x0080_0000, # Only used by Gauss
	
	Shield		= 0x0100_0000, # Shield
	Stationary	= 0x0200_0000, # Weapon does not pivot up and down
	Unmounted	= 0x0400_0000, # Used by MM and MM-Dummy
	Mounted		= 0x0800_0000, # Wether or not a shoulder weapon actually occupies a mounting point
	
	Fixed		= 0x1000_0000,
}

enum Bones {
	Muzzle,
	Ejector,
	Barrel,
	Recoil,
}

static func get_special(bone: Bones) -> String:
	return "special_%d" % bone

signal fired()
signal charging()
signal recoil_changed()

@export var config: BBWeaponConfig
@export_flags("Shoulder:65536","AttackAnim:131072","EquipAnim:262144","Melee:524288","Shield2:2097152","Gauss:8388608","Shield:16777216","Stationary:33554432","Unmounted:67108864","Mounted:134217728","Fixed:268435456") var flags: int

@export var model: Node3D
@export var material: Material

var skeleton: Skeleton3D

@export var mech_effect: BBEffectConfigGroup
@export var charging_effect: BBEffectConfigGroup
@export var firing_effects: Array[BBEffectConfigGroup]
@export var smoke_effect: BBEffectConfigGroup
@export var smoke_effect_count: int = 5
@export var smoke_effect_interval: float = 0.1
@export var casing_effect: BBEffectConfigGroup

@export var flash_count: int
@export var flashes: Array[BBPointLight]
@export_range(0.0, 1.0, 0.01) var recoil: float:
	set(val):
		recoil = clampf(val, 0.0, 1.0)
		recoil_changed.emit()

var mech_muzzles: Array[Marker3D]

var _smoke_effect_remaining: int
var _smoke_effect_timer: float

var _barrel_rotation_speed: float = 0.0
var _barrel_rotation_angle: float = 0.0

var equipped: bool
var firing: bool

func _ready() -> void:
	skeleton = model.get_node(^"Skeleton3D") as Skeleton3D
	var mesh_inst := skeleton.get_node(^"0") as MeshInstance3D
	mesh_inst.material_override = material

func _process(delta: float) -> void:
	var recoil_idx := skeleton.find_bone(get_special(Bones.Recoil))
	if recoil_idx >= 0:
		var recoil_trans := skeleton.get_bone_rest(recoil_idx)
		recoil_trans = recoil_trans.translated_local(Vector3.FORWARD * recoil)
		skeleton.set_bone_pose(recoil_idx, recoil_trans)
	
	var barrel_idx := skeleton.find_bone(get_special(Bones.Barrel))
	if barrel_idx >= 0:
		_barrel_rotation_speed = move_toward(_barrel_rotation_speed, 50.0 if firing else 0.0, 100.0 * delta)
		_barrel_rotation_angle = fmod(_barrel_rotation_angle + _barrel_rotation_speed * delta, TAU)
		
		var barrel_trans := skeleton.get_bone_rest(barrel_idx)
		barrel_trans = barrel_trans.rotated_local(Vector3.BACK, _barrel_rotation_angle)
		skeleton.set_bone_pose(barrel_idx, barrel_trans)
	
	if smoke_effect && _smoke_effect_remaining > 0:
		_smoke_effect_timer -= delta
		if _smoke_effect_timer <= 0.0:
			_smoke_effect_timer += smoke_effect_interval
			_smoke_effect_remaining -= 1
			
			var smoke_effect_args := {
				"attach_node": skeleton,
				"attach_bone": get_special(Bones.Muzzle),
				"detach_delay": 0.0,
			}
			
			BBEffectManager.spawn(smoke_effect, smoke_effect_args)

func set_weapon_quaternion(quat: Quaternion) -> void:
	if flags & Flags.Stationary == Flags.Stationary:
		return
	
	if flags & Flags.Shoulder == Flags.Shoulder:
		var pivot_idx := skeleton.find_bone("2")
		assert(pivot_idx >= 0)
		skeleton.set_bone_pose_rotation(pivot_idx, quat)
		return
	
	model.quaternion = quat

func get_weapon_quaternion() -> Quaternion:
	if flags & Flags.Shoulder == Flags.Shoulder:
		var pivot_idx = skeleton.find_bone("2")
		assert(pivot_idx >= 0)
		return skeleton.get_bone_pose_rotation(pivot_idx)
	
	return model.quaternion

func _get_mech_muzzle(muzzle: int) -> Marker3D:
	if mech_muzzles.is_empty():
		return null
	return mech_muzzles[muzzle % mech_muzzles.size()]

func get_muzzle_transform(muzzle: int) -> Transform3D:
	if flags & Flags.Unmounted == Flags.Unmounted:
		var muzzle_marker := _get_mech_muzzle(muzzle)
		return muzzle_marker.global_transform if muzzle_marker else Transform3D.IDENTITY
	
	var muzzle_bone := skeleton.find_bone(get_special(Bones.Muzzle))
	assert(muzzle_bone >= 0)
	
	var offset := config.get_muzzle_offset(muzzle)
	return skeleton.global_transform * skeleton.get_bone_global_pose(muzzle_bone).translated_local(offset)

func charge(muzzle: int = 0) -> void:
	if charging_effect:
		var muzzle_offset := config.get_muzzle_offset(muzzle)
		var charging_effect_args := {
			"attach_node": skeleton,
			"attach_bone": get_special(Bones.Muzzle),
			"position": muzzle_offset,
		}
		
		BBEffectManager.spawn(charging_effect, charging_effect_args, self)
	
	charging.emit()

func fire(muzzle: int = 0) -> void:
	var firing_effect_args: Dictionary
	if flags & Flags.Unmounted == Flags.Unmounted:
		firing_effect_args = {
			"attach_node": _get_mech_muzzle(muzzle),
			"detach_delay": 0.0,
		}
	else:
		firing_effect_args = {
			"attach_node": skeleton,
			"attach_bone": get_special(Bones.Muzzle),
			"detach_delay": 0.0,
			"position": config.get_muzzle_offset(muzzle),
		}
	
	for firing_effect in firing_effects:
		BBEffectManager.spawn(firing_effect, firing_effect_args)
	
	if smoke_effect:
		_smoke_effect_remaining = smoke_effect_count
		_smoke_effect_timer = 0.0
	
	if casing_effect:
		var casing_effect_args := {
			"attach_node": skeleton,
			"detach_delay": 0.0,
		}
		
		var ejector_bone := get_special(Bones.Ejector)
		if skeleton.find_bone(ejector_bone) >= 0:
			casing_effect_args.attach_bone = ejector_bone
		else:
			# The Ejector Bone is missing, so approximate it's position based on the muzzle
			var muzzle_bone_idx := skeleton.find_bone(get_special(Bones.Muzzle))
			var muzzle_rest := skeleton.get_bone_global_rest(muzzle_bone_idx)
			casing_effect_args.position = Vector3(0.0, muzzle_rest.origin.y, 0.0)
		
		BBEffectManager.spawn(casing_effect, casing_effect_args)
	
	if !flashes.is_empty():
		var flash_idx := (muzzle * flash_count) % flashes.size()
		flash_idx += randi() % flash_count
		flashes[flash_idx].start()
	
	fired.emit()
