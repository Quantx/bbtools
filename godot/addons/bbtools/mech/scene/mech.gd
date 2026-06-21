class_name BBMech extends Node3D

enum Bones {
	Torso,		# special_0
	MwepRight,	# special_1
	MwepLeft,	# special_2
	SwepBox,	# special_3
	SwepRight,	# special_4
	SwepLeft,	# special_5
	SwepCenter,	# special_6
	Manipulator,# special_7
	SubCamFront,# special_8
	MainCam,	# special_9
	TankRight,	# special_10
	TankLeft,	# special_11
	Cockpit,	# special_12
	MwepCenter,	# special_13
	FootRight,	# special_14
	FootLeft,	# special_15
	Emblem,		# special_16
	SubCamBack,	# special_17
}

static func get_special(bone: Bones) -> String:
	return "special_%d" % bone

static func _update_attach_transform_bone_name(node: Node3D, skeleton: Skeleton3D, bone_name: String) -> void:
	if !is_instance_valid(node):
		return
	
	if !skeleton.is_inside_tree():
		return
	
	var bone_idx := skeleton.find_bone(bone_name)
	if bone_idx >= 0:
		node.global_transform = skeleton.global_transform * skeleton.get_bone_global_pose(bone_idx)

static func _update_attach_transform(node: Node3D, skeleton: Skeleton3D, bone: Bones) -> void:
	_update_attach_transform_bone_name(node, skeleton, get_special(bone))

@export var config: BBMechConfig

@export var mech_material: ShaderMaterial
func _apply_mech_material(node: Node) -> void:
	var mesh_inst := node.get_node(^"Skeleton3D/0") as MeshInstance3D
	mesh_inst.material_override = mech_material

func set_paint_colors(colors: PackedColorArray) -> void:
	if colors.size() != BBMechConfig.paint_area_count:
		push_error("Cannot set mech paint colors, need %d colors" % BBMechConfig.paint_area_count)
		return
	
	# All colors must be opaque
	for i in BBMechConfig.paint_area_count:
		colors[i].a = 1.0
	
	colors.append(Color.TRANSPARENT) # Final color must be transparent
	mech_material.set_shader_parameter("paint_colors", colors)

func is_paint_applied() -> bool:
	var paint_colors := mech_material.get_shader_parameter("paint_colors")
	if !paint_colors:
		return false
	
	# Can be either type for some reason
	if paint_colors[0] is Vector4:
		var first := paint_colors[0] as Vector4
		return first.w > 0.0
	elif paint_colors[0] is Color:
		var first := paint_colors[0] as Color
		return first.a > 0.0
	
	return false

@export var emblem_material: ShaderMaterial
func set_emblem_texture(texture: Texture2D) -> void:
	emblem_material.set_shader_parameter("albedo_texture", texture)
	emblem_material.set_shader_parameter("alpha_scale", 1.0 if texture else 0.0)

# Mech camo meshes
@export var chassis_meshes: Array[Mesh]
@export var hatch_meshes: Array[Mesh]

var mesh_idx: int:
	set = set_mesh_idx
func set_mesh_idx(new_idx: int) -> void:
	mesh_idx = clampi(new_idx, 0, chassis_meshes.size() - 1)
	
	var chassis_mesh_inst := chassis.get_node(^"Skeleton3D/0") as MeshInstance3D
	chassis_mesh_inst.mesh = chassis_meshes[mesh_idx]
	
	var hatch_mesh_inst := hatch.get_node(^"Skeleton3D/0") as MeshInstance3D
	hatch_mesh_inst.mesh = hatch_meshes[mesh_idx]

# Mech parts
@export var chassis: Node3D
@export var hatch: Node3D
@export var emblem: Node3D
@export var manipulator: Node3D
@export var mwep_mount_right: Node3D
@export var mwep_mount_left: Node3D
@export var swep_mount_box: Node3D

# Animation Trees
@export var chassis_anim_tree: AnimationTree
@export var hatch_anim_tree: AnimationTree
@export var manipulator_anim_tree: AnimationTree
@export var mwep_right_anim_tree: AnimationTree
@export var mwep_left_anim_tree: AnimationTree
@export var swep_mount_box_anim_tree: AnimationTree

# Mech eye effect
@export var eye_effect_config: BBEffectConfigGroup
@export var eye_effect_position: Vector3

# Mech movement collider
@export var movement_collider_shape: CapsuleShape3D
@export var movement_collider_offset: Vector3

# Skeleton Modifers
@export var chassis_skel_mod: BBSkeletonModiferChassis
@export var chassis_jettison_skel_mod: BBSkeletonModiferJettison
@export var hatch_jettison_skel_mod: BBSkeletonModiferJettison

# Attachment points
@export var torso: Marker3D

@export var camera_main: Marker3D
@export var camera_sub_front: Marker3D
@export var camera_sub_back: Marker3D

@export var swep_attachments_box: Marker3D
@export var swep_attachments_shoulder: Array[Marker3D] # Size 2
@export var mwep_attachments: Array[Marker3D] # Size 4

@export var muzzle_bones: PackedStringArray
@export var muzzles: Array[Marker3D]

var _swep_shoulder_count := 0

# Weapons
var mweps: Array[BBWeapon] = [null, null, null, null]
var sweps: Array[BBWeapon] = [null, null, null]

var mwep_idx: int
var swep_idx: int

var hatch_closed: bool
var manipulator_deployed: bool

func _ready() -> void:
	# Setup Mesh Instances
	_apply_mech_material(chassis)
	_apply_mech_material(hatch)
	_apply_mech_material(manipulator)
	_apply_mech_material(mwep_mount_right)
	_apply_mech_material(mwep_mount_left)
	_apply_mech_material(swep_mount_box)
	if !is_paint_applied():
		set_paint_colors(config.palettes[mesh_idx].colors)
	
	var emblem_mesh_inst := emblem.get_node(^"Skeleton3D/0") as MeshInstance3D
	emblem_mesh_inst.material_override = emblem_material
	
	# Setup Skeletons
	var chassis_skeleton := chassis.get_node(^"Skeleton3D") as Skeleton3D
	chassis_skeleton.skeleton_updated.connect(_on_skeleton_updated, CONNECT_APPEND_SOURCE_OBJECT)
	
	var hatch_skeleton := hatch.get_node(^"Skeleton3D") as Skeleton3D
	hatch_skeleton.skeleton_updated.connect(_on_skeleton_updated, CONNECT_APPEND_SOURCE_OBJECT)
	
	var swep_mount_box_skeleton := swep_mount_box.get_node(^"Skeleton3D") as Skeleton3D
	swep_mount_box_skeleton.skeleton_updated.connect(_on_swep_box_skeleton_update, CONNECT_APPEND_SOURCE_OBJECT)
	
	var mwep_mount_right_skeleton := mwep_mount_right.get_node(^"Skeleton3D") as Skeleton3D
	mwep_mount_right_skeleton.skeleton_updated.connect(_on_mwep_mount_skeleton_updated.bind(false), CONNECT_APPEND_SOURCE_OBJECT)
	mwep_mount_right_skeleton.get_node(^"0").hide()
	
	var mwep_mount_left_skeleton := mwep_mount_left.get_node(^"Skeleton3D") as Skeleton3D
	mwep_mount_left_skeleton.skeleton_updated.connect(_on_mwep_mount_skeleton_updated.bind(true), CONNECT_APPEND_SOURCE_OBJECT)
	mwep_mount_left_skeleton.get_node(^"0").hide()
	
	var swep_mount_box_anim_tree_playback := swep_mount_box_anim_tree.get("parameters/playback") as AnimationNodeStateMachinePlayback
	swep_mount_box_anim_tree_playback.state_started.connect(_on_swep_box_animation_tree_state_started)
	
	# Connect Animation Trees to Animation Sequences
	for n in get_children():
		var anim_tree := n.get_node_or_null(^"AnimationTree") as AnimationTree
		var anim_sequence := n.get_node_or_null(^"AnimationSequence") as BBAnimationSequence
		if anim_tree && anim_sequence:
			anim_tree.mixer_applied.connect(anim_sequence.on_mixer_applied)
	
	if !Engine.is_editor_hint():
		var eye_effect_args := {
			"position": eye_effect_position,
			"attach_node": chassis_skeleton,
			"attach_bone": get_special(Bones.MainCam)
		}
		
		BBEffectManager.spawn(eye_effect_config, eye_effect_args, self)

func spawn_movement_collider() -> CollisionShape3D:
	var collision_shape := CollisionShape3D.new()
	collision_shape.name = "MovementCollider"
	collision_shape.shape = movement_collider_shape
	collision_shape.position = movement_collider_offset
	return collision_shape

# Called by both chassis & hatch skeleton
func _on_skeleton_updated(skeleton: Skeleton3D) -> void:
	_update_attach_transform(hatch, skeleton, Bones.Cockpit)
	_update_attach_transform(emblem, skeleton, Bones.Emblem)
	_update_attach_transform(manipulator, skeleton, Bones.Manipulator)
	_update_attach_transform(mwep_mount_right, skeleton, Bones.MwepRight)
	_update_attach_transform(mwep_mount_left, skeleton, Bones.MwepLeft)
	_update_attach_transform(swep_mount_box, skeleton, Bones.SwepBox)
	
	assert(muzzles.size() == muzzle_bones.size())
	for i in muzzles.size():
		_update_attach_transform_bone_name(muzzles[i], skeleton, muzzle_bones[i])
	
	# Flip left MWep mount
	mwep_mount_left.scale = Vector3(-1.0, 1.0, 1.0)
	
	_update_attach_transform(torso, skeleton, Bones.Torso)
	
	_update_attach_transform(camera_main, skeleton, Bones.MainCam)
	_update_attach_transform(camera_sub_front, skeleton, Bones.SubCamFront)
	_update_attach_transform(camera_sub_back, skeleton, Bones.SubCamBack)
	
	_update_attach_transform(swep_attachments_shoulder[0], skeleton, Bones.SwepLeft)
	_update_attach_transform(swep_attachments_shoulder[1], skeleton, Bones.SwepRight)

func _on_swep_box_skeleton_update(skeleton: Skeleton3D) -> void:
	if !skeleton.is_inside_tree():
		return
	
	var bone_idx := skeleton.find_bone("special_0")
	assert(bone_idx >= 0)
	
	swep_attachments_box.global_transform = skeleton.global_transform * skeleton.get_bone_global_pose(bone_idx)

func _on_mwep_mount_skeleton_updated(skeleton: Skeleton3D, is_left: bool) -> void:
	if !skeleton.is_inside_tree():
		return
	
	var bone_idx_0 := skeleton.find_bone("special_0")
	assert(bone_idx_0 >= 0)
	
	var bone_idx_1 := skeleton.find_bone("special_1")
	assert(bone_idx_1 >= 0)
	
	var attach_idx_0 := int(is_left)
	var attach_idx_1 := attach_idx_0 + 2
	
	var mounts: Array[Marker3D] = [mwep_attachments[attach_idx_0], mwep_attachments[attach_idx_1]]
	
	mounts[0].global_transform = skeleton.global_transform * skeleton.get_bone_global_pose(bone_idx_0)
	mounts[1].global_transform = skeleton.global_transform * skeleton.get_bone_global_pose(bone_idx_1)
	
	for attachment in mounts:
		attachment.scale = Vector3.ONE
		if is_left:
			attachment.rotate_x(PI)

#region weapons
func _create_weapon(weapon_config: BBWeaponConfig) -> BBWeapon:
	var weapon := weapon_config.weapon_scene.instantiate() as BBWeapon
	if weapon.flags & BBWeapon.Flags.Unmounted && muzzles.is_empty():
		weapon.queue_free()
		push_error("Weapon is unmounted, but muzzles is empty")
		return null
	
	weapon.fired.connect(_on_weapon_fired, CONNECT_APPEND_SOURCE_OBJECT)
	weapon.recoil_changed.connect(_on_recoil_changed, CONNECT_APPEND_SOURCE_OBJECT)
	weapon.mech_muzzles = muzzles
	return weapon

func _create_main_weapon(weapon_config: BBWeaponConfig, slot: int) -> BBWeapon:
	if mweps[slot]:
		mweps[slot].queue_free()
		mweps[slot] = null
	
	if !weapon_config:
		return null
	
	if weapon_config.type != BBWeaponConfig.WeaponType.MWEP:
		push_error("Expected main weapon", weapon_config)
		return null
	
	var weapon := _create_weapon(weapon_config)
	mweps[slot] = weapon
	return weapon

func set_main_weapon(weapon_config: BBWeaponConfig, slot: int) -> BBWeapon:
	var weapon := _create_main_weapon(weapon_config, slot)
	
	if weapon:
		mwep_attachments[slot].add_child(weapon)
	
	if slot >= 2:
		var mwep_mount := mwep_mount_right if slot % 2 == 0 else mwep_mount_left
		var mesh_inst := mwep_mount.get_node(^"Skeleton3D/0") as MeshInstance3D
		mesh_inst.visible = weapon != null
	
	return weapon

func _create_sub_weapon(weapon_config: BBWeaponConfig, slot: int) -> BBWeapon:
	if sweps[slot]:
		sweps[slot].queue_free()
		sweps[slot] = null
	
	if !weapon_config:
		return null
	
	if weapon_config.type != BBWeaponConfig.WeaponType.SWEP:
		push_error("Expected sub weapon", weapon_config)
		return null
	
	var weapon := _create_weapon(weapon_config)
	sweps[slot] = weapon
	return weapon

func _attach_shoulder_weapons(fixed: bool) -> void:
	for w in sweps:
		if w && w.flags & BBWeapon.Flags.Shoulder && bool(w.flags & BBWeapon.Flags.Fixed) == fixed:
			if _swep_shoulder_count < swep_attachments_shoulder.size():
				swep_attachments_shoulder[_swep_shoulder_count].add_child(w)
			
			# Add to count of shoulder weapons even if it couldn't be attached to detect overflows
			if w.flags & BBWeapon.Flags.Mounted:
				_swep_shoulder_count += 1

func _reattach_shoulder_weapons() -> void:
	for w in sweps:
		if !w:
			continue
		
		# Detach shoulder weapons so that they can be re-organized
		var wp := w.get_parent()
		if wp && w.flags & BBWeapon.Flags.Shoulder:
			wp.remove_child(w)
	
	# Attach shoulder weapons (fixed weapons are always attached first)
	_swep_shoulder_count = 0
	_attach_shoulder_weapons(true)
	_attach_shoulder_weapons(false)

func set_sub_weapon(weapon_config: BBWeaponConfig, slot: int) -> BBWeapon:
	var weapon := _create_sub_weapon(weapon_config, slot)
	
	if weapon && !(weapon.flags & BBWeapon.Flags.Shoulder):
		weapon.hide()
		swep_attachments_box.add_child(weapon)
	else:
		_reattach_shoulder_weapons()
	
	return weapon

func is_shoulder_weapon_overflow() -> bool:
	return _swep_shoulder_count >= swep_attachments_shoulder.size()

func is_sub_weapon_boxed(weapon: BBWeapon = sweps[swep_idx]) -> bool:
	return weapon && weapon.flags & BBWeapon.Flags.Shoulder != BBWeapon.Flags.Shoulder

func should_swep_box_open() -> bool:
	return is_sub_weapon_boxed()

func should_swep_box_close() -> bool:
	return !is_sub_weapon_boxed() || !sweps[swep_idx].visible

func _on_swep_box_animation_tree_state_started(state: StringName) -> void:
	# Hide all boxed SWEPs weapons
	if state == &"Anim_0":
		for w in sweps:
			if is_sub_weapon_boxed(w):
				w.hide()
	
	# Show the current SWEP if it's boxed
	if state == &"Anim_1" && is_sub_weapon_boxed():
		sweps[swep_idx].show()

func _on_weapon_fired(weapon: BBWeapon) -> void:
	if weapon.mech_effect:
		var mech_effect_args := {
			"attach_node": self,
			"detach_delay": 0.0,
		}
		
		BBEffectManager.spawn(weapon.mech_effect, mech_effect_args)

func _on_recoil_changed(weapon: BBWeapon) -> void:
	var first_recoil_weapon: BBWeapon = null
	for w in sweps:
		if w && w.config.mech_recoil:
			first_recoil_weapon = w
			break
	
	if first_recoil_weapon == weapon:
		chassis_skel_mod.recoil_weight = weapon.recoil
#endregion

#region movement
var deployed: bool
@export var movement_anim_speeds: Dictionary[StringName,float]
func set_movement(movement_node: StringName, speed_ms: float = -1.0) -> void:
	if movement_node in movement_anim_speeds:
		if speed_ms < 0.0:
			push_error("Speed argument is required for movement node %s" % movement_node)
			return
		
		var anim_speed := movement_anim_speeds[movement_node]
		chassis_anim_tree.set("parameters/%s/TimeScale/scale" % movement_node, speed_ms / anim_speed)
	
	var movement_playback := chassis_anim_tree.get("parameters/playback") as AnimationNodeStateMachinePlayback
	movement_playback.travel(movement_node)

func set_slide(slide_node: StringName) -> void:
	var movement_playback := chassis_anim_tree.get("parameters/playback") as AnimationNodeStateMachinePlayback
	movement_playback.travel(&"Slide")
	
	var slide_playback := chassis_anim_tree.get("parameters/Slide/playback") as AnimationNodeStateMachinePlayback
	slide_playback.travel(slide_node)

var rising: bool
func set_fall(fall_node: StringName) -> void:
	var movement_playback := chassis_anim_tree.get("parameters/playback") as AnimationNodeStateMachinePlayback
	movement_playback.travel(&"Fall") 
	
	var fall_playback := chassis_anim_tree.get("parameters/Fall/playback") as AnimationNodeStateMachinePlayback
	fall_playback.travel(fall_node)

func is_standing() -> bool:
	var movement_playback := chassis_anim_tree.get("parameters/playback") as AnimationNodeStateMachinePlayback
	return movement_playback.get_current_node() != &"Fall"

func is_deployed() -> bool:
	var deploy_playback := chassis_anim_tree.get("parameters/Deploy/playback") as AnimationNodeStateMachinePlayback
	return deploy_playback.get_current_node() == &"Idle"
#endregion

#region equiptment
# Left tank is dropped first, then right
@export var tank_right_jettision_direction: Vector3
@export var tank_left_jettison_direction: Vector3
func drop_sub_tank(right: bool, speed: float = 5.0) -> void:
	var bone_name := get_special(Bones.TankRight if right else Bones.TankLeft)
	var velocity := (tank_right_jettision_direction if right else tank_left_jettison_direction) * speed
	chassis_jettison_skel_mod.jettison_bone(bone_name, velocity)

@export var chassis_opt_armor_bones: PackedStringArray
@export var hatch_opt_armor_bones: PackedStringArray
func drop_opt_armor(speed: float = 5.0) -> void: 
	for bone_name in chassis_opt_armor_bones:
		chassis_jettison_skel_mod.jettison_bone(bone_name, Vector3.BACK * speed)
	
	for bone_name in hatch_opt_armor_bones:
		hatch_jettison_skel_mod.jettison_bone(bone_name, Vector3.BACK * speed)

func set_torso_rotation_y(angle: float) -> void:
	chassis_skel_mod.torso_rotation_y = angle
#endregion

func get_projectile_hit_effect_args(bone_idx: int = -1) -> Dictionary:
	const hit_bones: PackedInt32Array = [
		Bones.Torso,		# special_0
		Bones.MwepRight,	# special_1
		Bones.MwepLeft,		# special_2
		Bones.SwepBox,		# special_3
		Bones.SwepRight,	# special_4
		Bones.SwepLeft,		# special_5
		Bones.SwepCenter,	# special_6
		Bones.Manipulator,	# special_7
		
		Bones.TankRight,	# special_10
		Bones.TankLeft,		# special_11
		
		Bones.MwepCenter,	# special_13
		Bones.FootRight,	# special_14
		Bones.FootLeft,		# special_15
		Bones.Emblem,		# special_16
	]
	
	if bone_idx < 0:
		bone_idx = randi()
	
	bone_idx %= hit_bones.size()
	
	return {
		"attach_node": chassis.get_node(^"Skeleton3D") as Skeleton3D,
		"attach_bone": get_special(hit_bones[bone_idx]),
		"detach_delay": 0.0,
	}
