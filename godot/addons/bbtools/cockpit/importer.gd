@tool
class_name EditorSceneFormatImporterBBCockpit extends EditorSceneFormatImporter

const background_distance := 20.0

const monitor_names: PackedStringArray = [
	"Background",
	"Main",
	"Sub",
	"Multi"
]

const chassis_names: PackedStringArray = [
	"Hull",
	"Overlay",
	"",
	"Hatch",
]

func _get_extensions() -> PackedStringArray:
	return ["cockpit_scene"]

func _get_import_flags() -> int:
	return IMPORT_SCENE

func _import_scene(path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	
	var cockpit := BBCockpit.new()
	cockpit.name = "Cockpit"
	cockpit.top_level = true
	
	var _id := file.get_8()
	
	var cockpit_offset: Vector3
	cockpit_offset.x = file.get_float()
	cockpit_offset.y = file.get_float()
	cockpit_offset.z = file.get_float()
	
	var pilot := Node3D.new()
	pilot.name = "Pilot"
	
	cockpit.add_child(pilot)
	pilot.owner = cockpit
	
	cockpit.pilot_root = pilot
	
	var pose_count := file.get_8()
	for i in pose_count:
		var pose := Marker3D.new()
		pose.name = "Pose_%d" % i
		
		pose.position.x = file.get_float()
		pose.position.y = file.get_float()
		pose.position.z = file.get_float()
		
		pose.rotation.x = file.get_float()
		pose.rotation.y = file.get_float()
		pose.rotation.z = file.get_float()
		
		pilot.add_child(pose)
		pose.owner = cockpit
		
		cockpit.pilot_poses.append(pose)
	
	var environment := Environment.new()
	environment.background_mode = Environment.BG_COLOR
	environment.background_color = Color.BLACK
	environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	
	environment.fog_enabled = true
	environment.fog_mode = Environment.FOG_MODE_DEPTH
	environment.fog_light_color = Color.BLACK
	environment.fog_depth_begin = background_distance + 10.0
	environment.fog_depth_end = background_distance + 20.0
	
	var camera := Camera3D.new()
	camera.name = "Camera"
	camera.fov = 60.0
	camera.cull_mask = 2
	camera.far = 100.0
	camera.transform = cockpit.pilot_poses[0].transform
	camera.environment = environment
	
	pilot.add_child(camera)
	camera.owner = cockpit
	
	cockpit.pilot_camera = camera
	
	for i in 2:
		var chassis_texture_path := file.get_pascal_string()
		cockpit.chassis_textures.append(load(chassis_texture_path) as Texture2D)
	
	var chassis_material := StandardMaterial3D.new()
	chassis_material.albedo_texture = cockpit.chassis_textures[0]
	chassis_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
	chassis_material.alpha_scissor_threshold = 0.5
	chassis_material.alpha_antialiasing_mode = BaseMaterial3D.ALPHA_ANTIALIASING_OFF
	
	cockpit.chassis_material = chassis_material
	
	for i in 2:
		var display_texture_path := file.get_pascal_string()
		cockpit.display_textures.append(load(display_texture_path) as Texture3D)
	
	var display_material := ShaderMaterial.new()
	display_material.shader = load("res://addons/bbtools/cockpit/shaders/display.gdshader") as Shader
	display_material.set_shader_parameter("albedo_texture", cockpit.display_textures[0])
	
	cockpit.display_material = display_material
	
	var background_material := ShaderMaterial.new()
	background_material.shader = load("res://addons/bbtools/cockpit/shaders/background.gdshader") as Shader
	
	cockpit.background_material = background_material
	
	var background_mesh := QuadMesh.new()
	background_mesh.size = Vector2.ONE * 100.0
	background_mesh.material = background_material
	
	var background := MeshInstance3D.new()
	background.name = "Background"
	background.mesh = background_mesh
	background.position = Vector3.FORWARD * background_distance
	background.layers = 2
	
	cockpit.add_child(background)
	background.owner = cockpit
	
	cockpit.background = background
	
	var lighting_path := file.get_pascal_string()
	var lighting_scene := load(lighting_path) as PackedScene
	var lighting_root := lighting_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as BBCockpitLighting
	
	lighting_root.environment = environment
	
	cockpit.add_child(lighting_root)
	lighting_root.owner = cockpit
	
	cockpit.lighting = lighting_root
	
	var lighting_anim_tree := _create_animation_tree(cockpit, lighting_root)
	if lighting_anim_tree:
		lighting_anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/lighting.tres") as AnimationNodeStateMachine
	
	var monitor_parts: Array[Node3D]
	
	var monitor_root := Node3D.new()
	monitor_root.name = "Monitor"
	
	cockpit.add_child(monitor_root)
	monitor_root.owner = cockpit
	monitor_root.position = cockpit_offset
	
	var monitor_count := file.get_8()
	for i in monitor_count:
		var model_path := file.get_pascal_string()
		var model_scene := load(model_path) as PackedScene
		var model := model_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
		
		var flags := file.get_32()
		
		if i == 0:
			model.hide()
		
		if monitor_names.get(i):
			model.name = monitor_names[i]
		
		monitor_root.add_child(model)
		model.owner = cockpit
		
		monitor_parts.append(model)
	
	var cockpit_parts: Array[Node3D]
	
	var chassis_root := Node3D.new()
	chassis_root.name = "Chassis"
	chassis_root.position = cockpit_offset
	
	cockpit.add_child(chassis_root)
	chassis_root.owner = cockpit
	
	var chassis_count := file.get_8()
	for i in chassis_count:
		var model_path := file.get_pascal_string()
		var model_scene := load(model_path) as PackedScene
		var model := model_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
		
		var flags := file.get_32()
		
		if i == 1 || i == 4:
			model.hide()
		
		if chassis_names.get(i):
			model.name = chassis_names[i]
		
		chassis_root.add_child(model)
		model.owner = cockpit
		
		cockpit_parts.append(model)
	
	var display_root := Node3D.new()
	display_root.name = "Display"
	display_root.position = cockpit_offset
	
	cockpit.add_child(display_root)
	display_root.owner = cockpit
	
	var display_count := file.get_8()
	for i in display_count:
		var model_path := file.get_pascal_string()
		var model_scene := load(model_path) as PackedScene
		var model := model_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
		
		var flags := file.get_32()
		
		display_root.add_child(model)
		model.owner = cockpit
		
		cockpit_parts.append(model)
	
	cockpit.monitor_main = monitor_parts[1]
	cockpit.monitor_sub = monitor_parts[2]
	cockpit.monitor_multi = monitor_parts[3]
	
	var startup: Array[Node3D] = monitor_parts.slice(1)
	startup.append_array(cockpit_parts.slice(0, -1))
	startup.remove_at(5)
	
	cockpit.pilot_eject = cockpit_parts[2]
	var ejector: Array[Node3D] = [
		cockpit_parts[2],
		cockpit_parts[-1],
	]
	
	var multi_monitors: Array[Node3D] = [
		monitor_parts[3]
	]
	
	var monitor_idx_0 := file.get_8()
	if monitor_idx_0 != 0xFF:
		multi_monitors.append(cockpit_parts[monitor_idx_0])
	
	var monitor_idx_1 := file.get_8()
	if monitor_idx_1 != 0xFF:
		multi_monitors.append(cockpit_parts[monitor_idx_1])
	
	var mweps: Array[Node3D]
	_append_part(file, cockpit_parts, mweps, &"MWEP")
	_append_part(file, cockpit_parts, mweps, &"MWEP")
	
	var sweps: Array[Node3D]
	_append_part(file, cockpit_parts, sweps, &"SWEP")
	_append_part(file, cockpit_parts, sweps, &"SWEP")
	
	var tuner_node_idx := file.get_8()
	var tuner_bone := file.get_8()
	if tuner_node_idx != 0xFF:
		cockpit.tuner_root = cockpit_parts[tuner_node_idx]
		cockpit.tuner_root.name = "Tuner"
		if tuner_bone != 0xFF:
			var tuner_skel_mod := BBSkeletonModifierTuner.new()
			tuner_skel_mod.name = "TunerSkeletonModifier"
			tuner_skel_mod.bone = "%d" % tuner_bone
			
			var tuner_anim_player := cockpit.tuner_root.get_node(^"AnimationPlayer") as AnimationPlayer
			if tuner_anim_player:
				var tuner_bone_path := "Skeleton3D:%d" % tuner_bone
				var tuner_anim := tuner_anim_player.get_animation("Anim_3")
				for t in tuner_anim.get_track_count():
					if tuner_anim.track_get_type(t) == Animation.TYPE_POSITION_3D && tuner_anim.track_get_path(t) == NodePath(tuner_bone_path):
						var key_count := tuner_anim.track_get_key_count(t)
						
						var start := tuner_anim.track_get_key_value(t, 0) as Vector3
						var end := tuner_anim.track_get_key_value(t, key_count - 1) as Vector3
						
						var delta := end - start
						
						tuner_skel_mod.travel = delta / 4.0
						break
			
			cockpit.tuner_root.add_child(tuner_skel_mod)
			tuner_skel_mod.owner = cockpit
			
			tuner_skel_mod.cockpit = cockpit
			cockpit.tuner = tuner_skel_mod
	
	var comms: Array[Node3D]
	_append_part(file, cockpit_parts, comms, &"Comms1")
	_append_part(file, cockpit_parts, comms, &"Comms2")
	
	var dials_node_idx := file.get_8()
	cockpit.dials_root = cockpit_parts[dials_node_idx]
	cockpit.dials_root.name = "Dials"
	
	var dials_skel_mod := BBSkeletonModifierDials.new()
	dials_skel_mod.name = "DialsSkeletonModifier"
	dials_skel_mod.max_speed_kmh = file.get_8()
	for i in BBDial.Type.size():
		var bone_idx := file.get_8()
		var dial_scale := file.get_float()
		
		if bone_idx == 0xFF:
			continue
		
		var dial := BBDial.new()
		dial.bone = "%d" % bone_idx
		dial.scale = dial_scale
		
		dials_skel_mod.dials[i] = dial
	
	cockpit.dials_root.add_child(dials_skel_mod)
	dials_skel_mod.owner = cockpit
	cockpit.dials = dials_skel_mod
	
	cockpit.indicator_effects.resize(BBCockpit.Indicators.size())
	cockpit.indicator_positions.resize(BBCockpit.Indicators.size())
	for i in BBCockpit.Indicators.size(): # 10
		if !file.get_8():
			continue
		
		var effect_path := file.get_pascal_string()
		cockpit.indicator_effects[i] = load(effect_path) as BBEffectConfigGroup
		cockpit.indicator_positions[i] = Vector3(file.get_float(), file.get_float(), file.get_float())
	
	if file.get_8(): # Comm lights present
		cockpit.comm_light_effects.resize(BBCockpit.comm_light_count)
		cockpit.comm_light_positions.resize(BBCockpit.comm_light_count)
		for i in BBCockpit.comm_light_count:
			var effect_path := file.get_pascal_string()
			cockpit.comm_light_effects[i] = load(effect_path) as BBEffectConfigGroup
			cockpit.comm_light_positions[i] = Vector3(file.get_float(), file.get_float(), file.get_float())
	
	for part in startup:
		var anim_tree := _create_animation_tree(cockpit, part)
		if !anim_tree:
			continue
		
		if part == cockpit.tuner_root && !cockpit.tuner:
			anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/tuner.tres") as AnimationNodeStateMachine
		elif part in comms:
			anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/comms.tres") as AnimationNodeStateMachine
		elif part in multi_monitors:
			anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/multi_monitor.tres") as AnimationNodeStateMachine
		elif part in mweps:
			anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/mwep.tres") as AnimationNodeStateMachine
		elif part in sweps:
			anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/swep.tres") as AnimationNodeStateMachine
		else:
			anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/startup.tres") as AnimationNodeStateMachine
	
	for part in ejector:
		var anim_tree := _create_animation_tree(cockpit, part)
		if !anim_tree:
			continue
		
		anim_tree.tree_root = load("res://addons/bbtools/cockpit/anim_state_machines/ejector.tres") as AnimationNodeStateMachine
	
	# This fixes an import issue, doesn't actually affect the scene itself
	for root in [cockpit, monitor_root, display_root, chassis_root]:
		for n in root.get_children():
			var anim_player := n.get_node_or_null(^"AnimationPlayer") as AnimationPlayer
			if anim_player:
				n.remove_child(anim_player)
				anim_player.queue_free()
	
	return cockpit

func _append_part(file: FileAccess, from: Array[Node3D], to: Array[Node3D], name: StringName) -> void:
	var part_idx := file.get_8()
	if part_idx != 0xFF:
		from[part_idx].name = name
		to.append(from[part_idx])

func _create_animation_tree(cockpit: BBCockpit, target: Node3D) -> AnimationTree:
	if !target.has_node(^"AnimationPlayer"):
		return null
	
	var anim_player := target.get_node(^"AnimationPlayer") as AnimationPlayer
	
	var anim_tree := AnimationTree.new()
	anim_tree.name = "AnimationTree"
	anim_tree.deterministic = false
	
	target.add_child(anim_tree)
	anim_tree.owner = cockpit
	
	anim_tree.advance_expression_base_node = anim_tree.get_path_to(cockpit)
	anim_tree.anim_player = anim_tree.get_path_to(anim_player)
	
	cockpit.animation_trees.append(anim_tree)
	
	return anim_tree
