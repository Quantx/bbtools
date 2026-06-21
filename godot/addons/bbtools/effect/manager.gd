extends Node3D

# https://ezcha.net/news/5-16-26-rendering-a-million-objects-in-godot

const mmAABBSize := 14000
const mmAABB := AABB(Vector3.ONE * (-0.5 * mmAABBSize), Vector3.ONE * mmAABBSize)

# These are the maximum number of VISIBLE effects, there can be more effects processing in the background
const effect2DWorldMax : int = 2048
const effect2DCockpitMax : int = 512

const effect3DWorldMax : int = 128

const shader_sprite_fog_path := "res://addons/bbtools/effect/shaders/sprite_fog.gdshader"
const shader_sprite_no_fog_path := "res://addons/bbtools/effect/shaders/sprite_no_fog.gdshader"

class RenderGroup extends RefCounted:
	var max_count: int
	var list: Array[BBEffect]
	var mesh: MultiMesh
	var instance_count: int
	var buffer: PackedFloat32Array
	
	func _init(_max_count : int) -> void:
		max_count = _max_count
		
		buffer.resize(max_count * BBEffect.buffer_size)
		
		mesh = MultiMesh.new()
		mesh.custom_aabb = mmAABB
		mesh.transform_format = MultiMesh.TRANSFORM_3D
		mesh.use_colors = true
		mesh.use_custom_data = true
		mesh.instance_count = max_count
		mesh.visible_instance_count = 0
	
	func clear() -> void:
		mesh.visible_instance_count = 0
		for effect in list:
			effect.free()
		list.clear()
	
	func register(effect : BBEffect) -> bool:
		if list.size() < max_count:
			list.append(effect)
			return true
		return false
	
	func get_permanent_count() -> int:
		return list.reduce(func(accum: int, effect: BBEffect) -> int: return accum + int(is_inf(effect.life)))
	
	func update(delta: float) -> void:
		# Update effects
		for i : int in range(list.size() - 1, -1, -1):
			var effect := list[i]
			if effect.is_done():
				# Remove the done effect and replace it with the last item in the array
				list[i] = list[-1]
				list.pop_back()
				
				effect.free()
			else:
				effect.update(delta)
		
		# Construct instance buffer
		instance_count = 0
		for i : int in list.size():
			if instance_count >= max_count:
				break
			
			var effect := list[i]
			if !effect.is_visible():
				continue
			
			# Copy buffer
			var off := instance_count * BBEffect.buffer_size
			for j : int in BBEffect.buffer_size:
				buffer[off + j] = effect.buffer[j]
			
			instance_count += 1
	
	func render() -> void:
		mesh.visible_instance_count = instance_count
		mesh.buffer = buffer
	
	func status_string() -> String:
		return "Processing %d / %d, Rendering %d / %d, Permanent %d" % [
			list.size(), max_count,
			mesh.visible_instance_count, mesh.instance_count,
			get_permanent_count()
		]

var groups_world: Dictionary[Resource,RenderGroup]
var node_world: Node3D

var groups_world_fog: Dictionary[Resource,RenderGroup]
var node_world_fog: Node3D
 
var groups_cockpit: Dictionary[Resource,RenderGroup]
var node_cockpit: Node3D

var surface_effects: PackedInt32Array

func _ready() -> void:
	physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	
	node_world = Node3D.new()
	node_world.name = "World"
	add_child(node_world)
	
	node_world_fog = Node3D.new()
	node_world_fog.name = "WorldFog"
	add_child(node_world_fog)
	
	node_cockpit = Node3D.new()
	node_cockpit.name = "Cockpit"
	add_child(node_cockpit)
	
	surface_effects = FileAccess.get_file_as_bytes("res://proprietary/loc/effects/surfaces/common.tbl").to_int32_array()

func reset() -> void:
	for group : RenderGroup in groups_world.values():
		group.clear()
	
	for group : RenderGroup in groups_world_fog.values():
		group.clear()
	
	for group : RenderGroup in groups_cockpit.values():
		group.clear()

func register(effect: BBEffect) -> void:
	# Set spawn function reference
	effect.child_effect_spawn_func = spawn
	effect.buffer.resize(BBEffect.buffer_size)
	
	var is_cockpit := (effect.config.flags & BBEffectConfig.Flags.Cockpit) == BBEffectConfig.Flags.Cockpit
	var no_fog := false
	
	var layers := 2 if is_cockpit else 1
	
	var groups := groups_cockpit
	var node := node_cockpit
	if !is_cockpit:
		no_fog = (effect.config.flags & BBEffectConfig.Flags.NoFog) == BBEffectConfig.Flags.NoFog
		groups = groups_world if no_fog else groups_world_fog
		node = node_world if no_fog else node_world_fog
	
	var res := effect.config.get_common_resource()
	var res_name := effect.config.get_common_name()
	
	if res not in groups:
		var multi_mesh_inst := MultiMeshInstance3D.new()
		multi_mesh_inst.name = res_name
		multi_mesh_inst.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		multi_mesh_inst.layers = layers
		multi_mesh_inst.material_override
		multi_mesh_inst.custom_aabb = mmAABB
		
		var render_group: RenderGroup
		if res is Texture2D:
			var sprite_material := ShaderMaterial.new()
			sprite_material.shader = load(shader_sprite_no_fog_path if no_fog else shader_sprite_fog_path) as Shader
			
			sprite_material.set_shader_parameter("albedo_texture", res as Texture2D)
			
			var quad_mesh := QuadMesh.new()
			quad_mesh.surface_set_material(0, sprite_material)
			
			render_group = RenderGroup.new(effect2DCockpitMax if is_cockpit else effect2DWorldMax)
			render_group.mesh.mesh = quad_mesh
		elif res is Mesh:
			var object_material := StandardMaterial3D.new()
			object_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
			object_material.albedo_texture = load("res://proprietary/loc/textures/SCIDOBJ.dds") as Texture2D
			object_material.disable_fog = no_fog
			multi_mesh_inst.material_override = object_material
			
			render_group = RenderGroup.new(effect3DWorldMax)
			render_group.mesh.mesh = res as Mesh
		else:
			push_error("Unsupported effect type")
			multi_mesh_inst.queue_free()
			return
		
		groups[res] = render_group
		
		multi_mesh_inst.multimesh = render_group.mesh
		
		node.add_child(multi_mesh_inst)
	
	if !groups[res].register(effect):
		push_warning("Effect %s overflow" % res_name)

func _process(delta: float) -> void:
	var root := get_tree().root
	
	### Update ###
	for group : RenderGroup in groups_world.values():
		group.update(delta)
	
	for group : RenderGroup in groups_world_fog.values():
		group.update(delta)
	
	for group : RenderGroup in groups_cockpit.values():
		group.update(delta)
	
	### Render ###
	for group : RenderGroup in groups_world.values():
		group.render()
	
	for group : RenderGroup in groups_world_fog.values():
		group.render()
	
	for group : RenderGroup in groups_cockpit.values():
		group.render()

func spawn(cfg: BBEffectConfigGroup, args: Dictionary = {}, effect_owner: Node = null) -> Array[BBEffect]:
	if "attach_node" in args:
		assert(args.attach_node is Node3D)
		if "attach_bone" in args:
			assert(args.attach_node is Skeleton3D)
	
	if "detach_delay" in args:
		assert(args.detach_delay as float >= 0.0)
	
	if "delay" in args:
		assert(args.delay as float >= 0.0)
	
	var t_quat: Quaternion
	if "trans_rotation" in args:
		t_quat = Quaternion.from_euler(args.trans_rotation as Vector3)
	
	var effects := cfg.instantiate()
	for effect in effects:
		if effect_owner:
			effect.owner = effect_owner
			effect.is_owned = true
		elif effect.life == INF:
			push_error("Effect with infinite life has no owner ", effect.config)
			effect.free()
			continue
		
		if "delay" in args:
			effect.timer -= args.delay as float
		
		if "position" in args:
			effect.initial_position += args.position as Vector3
		
		if "rotation" in args:
			effect.initial_rotation += args.rotation as Vector3
		
		if "trans_rotation" in args:
			effect.velocity_position *= t_quat
			effect.acceleration_position *= t_quat
		
		if "attach_node" in args:
			effect.attach_node = args.attach_node as Node3D
			
			if "attach_bone" in args:
				var skeleton := effect.attach_node as Skeleton3D
				effect.attach_bone_idx = skeleton.find_bone(args.attach_bone as String)
				effect.attach_bone_transform = skeleton.get_bone_global_pose(effect.attach_bone_idx)
				skeleton.skeleton_updated.connect(effect.on_attached_skeleton_updated, CONNECT_APPEND_SOURCE_OBJECT)
		
		if "detach_delay" in args:
			effect.detach_delay = args.detach_delay as float
		
		register(effect)
	
	return effects

func load_id(id: int) -> BBEffectConfigGroup:
	assert(id >= 0)
	var effect_path := "res://proprietary/loc/effects/effects/EFG%04d.efg" % id
	return load(effect_path) as BBEffectConfigGroup

func load_surface(surf_eff_idx: int, surface: BBHitbox.Surface, sequence_surface_effects: PackedInt32Array = []) -> BBEffectConfigGroup:
	assert(surface < BBHitbox.Surface.Max) # 28
	var surface_offset := surface * BBHitbox.Surface.Max + surf_eff_idx
	
	var id: int
	if surf_eff_idx < 10: # Yes, this should be 10 instead of 9. I have no idea why
		assert(surf_eff_idx < 9)
		
		assert(sequence_surface_effects.size() == BBAnimationSequence.expected_surface_effects_size)
		id = sequence_surface_effects[surface_offset + surf_eff_idx]
	else:
		assert(surf_eff_idx >= 9 && surf_eff_idx < 32) # First 9 entries are always zero
		id = surface_effects[surface_offset + surf_eff_idx]
	
	if id < 0:
		return null
	
	return load_id(id)
