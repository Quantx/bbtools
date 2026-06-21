@tool
class_name EditorSceneFormatImporterBBMech extends EditorSceneFormatImporter

func _get_extensions() -> PackedStringArray:
	return ["mech_scene"]

func _get_import_flags() -> int:
	return IMPORT_SCENE

func _import_scene(path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	
	var mech := BBMech.new()
	mech.name = "Mech"
	
	var id := file.get_8()
	var config_path := base_path.path_join("config.mech")
	mech.config = load(config_path) as BBMechConfig
	
	var texture := load("res://proprietary/loc/textures/SCIDOBJ.dds") as Texture2D
	
	var material := ShaderMaterial.new()
	material.resource_name = "MechMaterial"
	material.shader = load("res://addons/bbtools/mech/scene/mech.gdshader") as Shader
	material.set_shader_parameter("albedo_texture", texture)
	mech.mech_material = material
	
	for i in 4:
		var chassis_path := file.get_pascal_string()
		var chassis_scene := load(chassis_path) as PackedScene
		var chassis := chassis_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
		
		var chassis_mesh_inst := chassis.get_node(^"Skeleton3D/0") as MeshInstance3D
		mech.chassis_meshes.append(chassis_mesh_inst.mesh)
		
		var hatch_path := file.get_pascal_string()
		var hatch_scene := load(hatch_path) as PackedScene
		var hatch := hatch_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
		
		var hatch_mesh_inst := hatch.get_node(^"Skeleton3D/0") as MeshInstance3D
		mech.hatch_meshes.append(hatch_mesh_inst.mesh)
		
		if i == 0:
			chassis.name = "Chassis"
			mech.add_child(chassis)
			chassis.owner = mech
			mech.chassis = chassis
			
			hatch.top_level = true
			hatch.name = "Hatch"
			mech.add_child(hatch)
			hatch.owner = mech
			mech.hatch = hatch
		else:
			chassis.free()
			hatch.free()
	
	mech.set_editable_instance(mech.chassis, true)
	mech.set_editable_instance(mech.hatch, true)
	
	var emblem_material := ShaderMaterial.new()
	emblem_material.resource_name = "EmblemMaterial"
	emblem_material.shader = load("res://addons/bbtools/mech/scene/emblem.gdshader") as Shader
	mech.emblem_material = emblem_material
	
	var emblem_path :=  file.get_pascal_string()
	mech.emblem = _create_model(emblem_path, "Emblem", mech)
	
	var manipulator_path :=  file.get_pascal_string()
	mech.manipulator = _create_model(manipulator_path, "Manipulator", mech)
	
	const swep_mount_path := "res://proprietary/loc/models/SWEP_BOX.gltf"
	mech.swep_mount_box = _create_model(swep_mount_path, "SWepMountBox", mech)
	
	var mwep_mount_path := file.get_pascal_string()
	mech.mwep_mount_right = _create_model(mwep_mount_path, "MWepMountRight", mech)
	mech.mwep_mount_left = _create_model(mwep_mount_path, "MWepMountLeft", mech)
	
	var eye_effect_path := file.get_pascal_string()
	mech.eye_effect_config = load(eye_effect_path) as BBEffectConfigGroup
	
	mech.eye_effect_position.x = file.get_float()
	mech.eye_effect_position.y = file.get_float()
	mech.eye_effect_position.z = file.get_float()
	
	var movement_collider_size: Vector3
	movement_collider_size.x = file.get_float()
	movement_collider_size.y = file.get_float()
	movement_collider_size.z = file.get_float()
	
	mech.movement_collider_shape = CapsuleShape3D.new()
	mech.movement_collider_shape.height = movement_collider_size.y
	mech.movement_collider_shape.radius = maxf(movement_collider_size.x, movement_collider_size.z) * 0.5
	
	mech.movement_collider_offset.x = file.get_float()
	mech.movement_collider_offset.y = file.get_float()
	mech.movement_collider_offset.z = file.get_float()
	
	mech.torso = _create_marker("Torso", mech)
	
	mech.camera_main = _create_marker("CameraMain", mech)
	mech.camera_sub_front = _create_marker("CameraSubFront", mech)
	mech.camera_sub_back = _create_marker("CameraSubBack", mech)
	
	mech.swep_attachments_box = _create_marker("SWepAttachmentBox", mech)
	
	var chassis_skeleton := mech.chassis.get_node(^"Skeleton3D") as Skeleton3D
	
	var chassis_skel_mod := BBSkeletonModiferChassis.new()
	chassis_skel_mod.name = "ChassisSkeletonModifier"
	
	chassis_skeleton.add_child(chassis_skel_mod)
	chassis_skel_mod.owner = mech
	mech.chassis_skel_mod = chassis_skel_mod
	
	var chassis_jettison_skel_mod := BBSkeletonModiferJettison.new()
	chassis_jettison_skel_mod.name = "JettisonSkeletonModifier"
	
	chassis_skeleton.add_child(chassis_jettison_skel_mod)
	chassis_jettison_skel_mod.owner = mech
	mech.chassis_jettison_skel_mod = chassis_jettison_skel_mod
	
	for i in chassis_skeleton.get_bone_count():
		var bone_name := chassis_skeleton.get_bone_name(i)
		if chassis_skeleton.has_bone_meta(i, &"recoil"):
			var recoil := chassis_skeleton.get_bone_meta(i, &"recoil") as Array
			chassis_skel_mod.recoil_translation = Vector3(recoil[0], recoil[1], recoil[2])
			chassis_skel_mod.recoil_bone_name = bone_name
		
		if chassis_skeleton.has_bone_meta(i, &"armor"):
			mech.chassis_opt_armor_bones.append(bone_name)
		
		if chassis_skeleton.has_bone_meta(i, &"muzzle"):
			var muzzle := _create_marker("Muzzle%d" % mech.muzzles.size(), mech)
			mech.muzzles.append(muzzle)
			mech.muzzle_bones.append(bone_name)
	
	var tank_left_bone_idx := chassis_skeleton.find_bone(BBMech.get_special(BBMech.Bones.TankLeft))
	if tank_left_bone_idx >= 0:
		var tank_left_forward := chassis_skeleton.get_bone_global_rest(tank_left_bone_idx).basis.z
		mech.tank_left_jettison_direction = Vector3(0.0, 0.0, -signf(tank_left_forward.z))
	
	var tank_right_bone_idx := chassis_skeleton.find_bone(BBMech.get_special(BBMech.Bones.TankRight))
	if tank_right_bone_idx >= 0:
		var tank_right_forward := chassis_skeleton.get_bone_global_rest(tank_right_bone_idx).basis.z
		mech.tank_right_jettision_direction = Vector3(0.0, 0.0, -signf(tank_right_forward.z))
	
	var hatch_skeleton := mech.hatch.get_node(^"Skeleton3D") as Skeleton3D
	
	var hatch_jettison_skel_mod := BBSkeletonModiferJettison.new()
	hatch_jettison_skel_mod.name = "JettisonSkeletonModifier"
	
	hatch_skeleton.add_child(hatch_jettison_skel_mod)
	hatch_jettison_skel_mod.owner = mech
	mech.hatch_jettison_skel_mod = hatch_jettison_skel_mod
	
	for i in hatch_skeleton.get_bone_count():
		var bone_name := hatch_skeleton.get_bone_name(i)
		if hatch_skeleton.has_bone_meta(i, &"armor"):
			mech.hatch_opt_armor_bones.append(bone_name)
	
	for i in 2:
		var bone := BBMech.Bones.SwepLeft if i == 0 else BBMech.Bones.SwepRight
		
		var parent: Node3D
		if chassis_skeleton.find_bone(BBMech.get_special(bone)) >= 0:
			parent = mech.chassis
		elif hatch_skeleton.find_bone(BBMech.get_special(bone)) >= 0:
			parent = mech.hatch
		else:
			push_error("SWepAttachmentShoulder bone not defined in either chassis or hatch")
			continue
		
		mech.swep_attachments_shoulder.append(_create_marker("SWepAttachmentShoulder%d" % i, mech))
	
	for i in 4:
		var parent := mech.mwep_mount_right if i % 2 == 0 else mech.mwep_mount_left
		mech.mwep_attachments.append(_create_marker("MWepAttachment%d" % i, mech))
	
	# Compute Movement Animation Velocities
	var chassis_anim_player := mech.chassis.get_node(^"AnimationPlayer") as AnimationPlayer
	mech.movement_anim_speeds[&"Walk"] = _get_root_motion_velocity(chassis_anim_player, &"Anim_1")
	mech.movement_anim_speeds[&"Run"] = _get_root_motion_velocity(chassis_anim_player, &"Anim_2")
	mech.movement_anim_speeds[&"Reverse"] = _get_root_motion_velocity(chassis_anim_player, &"Anim_4")
	
	# Create Animation Trees
	mech.chassis_anim_tree = _create_animation_tree(mech, mech.chassis)
	mech.chassis_anim_tree.tree_root = load("res://addons/bbtools/mech/anim_state_machines/chassis.tres") as AnimationNodeStateMachine
	mech.chassis_anim_tree.root_motion_track = ^"Skeleton3D:0"
	
	mech.hatch_anim_tree = _create_animation_tree(mech, mech.hatch)
	mech.hatch_anim_tree.tree_root = load("res://addons/bbtools/mech/anim_state_machines/hatch.tres") as AnimationNodeStateMachine
	
	mech.mwep_right_anim_tree = _create_animation_tree(mech, mech.mwep_mount_right)
	mech.mwep_right_anim_tree.tree_root = load("res://addons/bbtools/mech/anim_state_machines/mwep_right.tres") as AnimationNodeStateMachine
	
	mech.mwep_left_anim_tree = _create_animation_tree(mech, mech.mwep_mount_left)
	mech.mwep_left_anim_tree.tree_root = load("res://addons/bbtools/mech/anim_state_machines/mwep_left.tres") as AnimationNodeStateMachine
	
	mech.swep_mount_box_anim_tree = _create_animation_tree(mech, mech.swep_mount_box)
	mech.swep_mount_box_anim_tree.tree_root = load("res://addons/bbtools/mech/anim_state_machines/swep_box.tres") as AnimationNodeStateMachine
	
	mech.manipulator_anim_tree = _create_animation_tree(mech, mech.manipulator)
	mech.manipulator_anim_tree.tree_root = load("res://addons/bbtools/mech/anim_state_machines/manipulator.tres") as AnimationNodeBlendTree
	
	# This fixes an import issue, doesn't actually affect the scene itself
	for n in mech.get_children():
		var anim_player := n.get_node_or_null(^"AnimationPlayer") as AnimationPlayer
		if anim_player:
			n.remove_child(anim_player)
			anim_player.queue_free()
	
	return mech

func _create_model(model_path: String, model_name: StringName, root: Node) -> Node3D:
	var model_scene := load(model_path) as PackedScene
	var model := model_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
	
	model.name = model_name
	
	root.add_child(model)
	model.owner = root
	return model

const marker_gizmo_extents := 1.0
func _create_marker(marker_name: StringName, root: Node) -> Marker3D:
	var marker := Marker3D.new()
	
	marker.name = marker_name
	marker.gizmo_extents = marker_gizmo_extents
	
	root.add_child(marker)
	marker.owner = root
	return marker

func _create_animation_tree(root: Node, target: Node3D) -> AnimationTree:
	if !target.has_node(^"AnimationPlayer"):
		return null
	
	var anim_player := target.get_node(^"AnimationPlayer") as AnimationPlayer
	
	var anim_tree := AnimationTree.new()
	anim_tree.name = "AnimationTree"
	anim_tree.deterministic = false
	
	target.add_child(anim_tree)
	anim_tree.owner = root
	
	anim_tree.advance_expression_base_node = anim_tree.get_path_to(root)
	anim_tree.anim_player = anim_tree.get_path_to(anim_player)
	
	return anim_tree

func _get_root_motion_velocity(anim_mixer: AnimationMixer, anim_name: StringName) -> float:
	var anim := anim_mixer.get_animation(anim_name)
	if !anim:
		push_error("Failed to get animation: %s" % anim_name)
		return NAN
	
	var track_idx := anim.find_track(^"Skeleton3D:0", Animation.TYPE_POSITION_3D)
	if track_idx < 0:
		push_error("Failed to get root motion track for animation: %s" % anim_name)
		return NAN
	
	var start := anim.position_track_interpolate(track_idx, 0)
	var end := anim.position_track_interpolate(track_idx, anim.length)
	return start.distance_to(end) / anim.length
