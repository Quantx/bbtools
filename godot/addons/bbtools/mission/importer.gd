@tool
class_name EditorSceneFormatImporterBBMission extends EditorSceneFormatImporter

func _get_extensions() -> PackedStringArray:
	return ["mission_scene"]

func _get_import_flags() -> int:
	return IMPORT_SCENE
	
func _import_scene(mission_path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := mission_path.get_base_dir()
	
	var environment = Environment.new()
	environment.resource_name = "MissionEnvironment"
	environment.fog_enabled = true
	environment.fog_sky_affect = 0.0
	environment.fog_mode = Environment.FOG_MODE_DEPTH
	environment.background_color = Color.BLACK
	environment.ambient_light_sky_contribution = 0.0
	
	var mission := BBMission.new()
	mission.name = "Mission"
	mission.top_level = true
	mission.environment = environment
	
	var sun := DirectionalLight3D.new()
	sun.name = "Sun"
	
	mission.sun = sun
	
	mission.add_child(sun)
	sun.owner = mission
	
	var world_environment := WorldEnvironment.new()
	world_environment.name = "WorldEnvironment"
	world_environment.environment = environment
	
	mission.add_child(world_environment)
	world_environment.owner = mission
	
	var mission_file := FileAccess.open(mission_path, FileAccess.READ)
	
	var stage_count := mission_file.get_32()
	if stage_count <= 0:
		return null
	
	var draw_terrain := mission_file.get_8() as bool
	
	var map_big_path := mission_file.get_pascal_string()
	if !map_big_path.is_empty():
		mission.map = load(map_big_path) as Texture2D
	
	var _map_small_path := mission_file.get_pascal_string() # Not used
	
	var object_texture_path := mission_file.get_pascal_string()
	if !object_texture_path.is_empty():
		var object_material := StandardMaterial3D.new()
		object_material.resource_name = "ObjectMaterial"
		object_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
		object_material.alpha_scissor_threshold = 0.3
		object_material.albedo_texture = load(object_texture_path) as Texture2D
		
		mission.object_material = object_material
	
	_import_objects(mission_file, mission)
	
	var start_pos := Vector3(mission_file.get_float(), mission_file.get_float(), mission_file.get_float())
	var start_yaw := mission_file.get_float()
	if !is_nan(start_pos.x + start_pos.y + start_pos.z + start_yaw):
		var spawn := Marker3D.new()
		spawn.name = "Spawn"
		spawn.position = start_pos
		spawn.rotation.y = start_yaw
		
		mission.add_child(spawn)
		spawn.owner = mission
	
	environment.background_mode = Environment.BG_SKY if draw_terrain else Environment.BG_COLOR
	if draw_terrain:
		var ground_path := base_path.path_join("terrain.ground")
		var ground_file := FileAccess.open(ground_path, FileAccess.READ)
		if !ground_file:
			push_error("Failed to open %s" % ground_path)
			mission.free()
			return null
		
		_import_ground(ground_file, base_path, environment, mission)
	
	for i in stage_count:
		var stage_path := base_path.path_join("tod%d.stage" % i)
		mission.stages.append(load(stage_path) as BBStage)
	mission.current_stage = stage_count - 1
	
	return mission

func _import_objects(file: FileAccess, mission: Node, full_import: bool = true) -> void:
	var object_root := Node3D.new()
	object_root.name = "Objects"
	
	mission.add_child(object_root)
	object_root.owner = mission
	
	var object_counts: Dictionary[int,int] = {}
	
	var mission_object_count := file.get_32()
	for i in mission_object_count:
		var model_path := file.get_pascal_string()
		var model_name = model_path.get_file().get_slice(".", 0)
		
		var object: BBObject
		if full_import:
			var model_scene := load(model_path) as PackedScene
			var model := model_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE)
			
			# This fixes an import issue, doesn't actually affect the scene itself
			var anim_player := model.get_node_or_null(^"AnimationPlayer") as AnimationPlayer
			if anim_player:
				model.remove_child(anim_player)
				anim_player.queue_free()
			
			model.set_script(load("res://addons/bbtools/mission/object.gd"))
			object = model as BBObject
		else:
			object = BBObject.new()
			object.scene_file_path = model_path
		
		object.life = file.get_16()
		object.id = file.get_16()
		
		object.flags = file.get_32()
		
		object.team_id = file.get_8()
		object.ticket_value = file.get_8()
		object.spawn_index = file.get_16()
		
		object.position = Vector3(file.get_float(), file.get_float(), file.get_float())
		object.quaternion = Quaternion(file.get_float(), file.get_float(), file.get_float(), file.get_float())
		
		# Give each object a unique name based on the number of times it's ID appears
		var object_idx := object_counts.get_or_add(object.id, 0)
		object.name = "%s_%d" % [model_name, object_idx]
		object_counts[object.id] += 1
		
		object_root.add_child(object)
		object.owner = mission

func _import_ground(file : FileAccess, base_path: String, environment: Environment, mission: Node):
	var sky_mode := file.get_8()
	var sky_texture_paths: PackedStringArray
	match sky_mode:
		0:
			sky_texture_paths.append(file.get_pascal_string())
			sky_texture_paths.append(file.get_pascal_string())
		1:
			sky_texture_paths.append(file.get_pascal_string())
	
	var terrain_texture_path := base_path.path_join(file.get_pascal_string())
	var ground_texture_path := file.get_pascal_string()
	
	var tilemap: Texture2D = null
	var tilemap_path := base_path.path_join("tilemap.dds")
	if ResourceLoader.exists(tilemap_path):
		tilemap = load(tilemap_path) as Texture2D
		assert(tilemap.get_format() == Image.FORMAT_R16I)
	
	var size: Vector2i
	size.x = file.get_32()
	size.y = file.get_32()
	
	var length := size.x * size.y
	
	var scale := file.get_float()
	
	var tilemap_size: Vector2i
	tilemap_size.x = file.get_32()
	tilemap_size.y = file.get_32()
	var tilemap_rotation := file.get_8() as bool;
	
	var terrain_heights := file.get_buffer(length * 4).to_float32_array()
	
	var quads_data := BBHitboxQuadsData.new()
	quads_data.layers = file.get_buffer(length * 2)
	quads_data.surfaces = file.get_buffer(length)
	
	for i in length:
		if quads_data.get_layer(i) == 0:
			terrain_heights[i] = NAN
	
	var water_heights := file.get_buffer(length * 4).to_float32_array()
	
	var sky_scale := Vector2(5000.0, 5000.0)
	
	var sky_material = ShaderMaterial.new()
	sky_material.resource_name = "SkyMaterial"
	sky_material.shader = load("res://addons/bbtools/mission/shaders/sky.gdshader") as Shader
	_set_cloud_mat_params(sky_material, sky_mode, sky_scale, sky_texture_paths)
	
	var sky := Sky.new()
	sky.sky_material = sky_material
	environment.sky = sky
	
	var terrain_mesh := _heightmap_generate_mesh(size, terrain_heights)
	if terrain_mesh:
		var terrain_material := ShaderMaterial.new()
		if tilemap:
			terrain_material.resource_name = "TerrainTileMapMaterial"
			terrain_material.shader = load("res://addons/bbtools/mission/shaders/terrain_tilemap.gdshader") as Shader
			terrain_material.set_shader_parameter("tilemap", tilemap)
			
			var ground_texture := load(ground_texture_path) as Texture2DArray
			terrain_material.set_shader_parameter("ground", ground_texture)
		else:
			terrain_material.resource_name = "TerrainMaterial"
			terrain_material.shader = load("res://addons/bbtools/mission/shaders/terrain.gdshader") as Shader
			
			var ground_texture := load(ground_texture_path) as Texture2D
			terrain_material.set_shader_parameter("ground", ground_texture)
		
		
		var terrain_texture := load(terrain_texture_path) as Texture2D
		terrain_material.set_shader_parameter("terrain", terrain_texture)
		
		terrain_material.set_shader_parameter("ground_scale", Vector2(tilemap_size))
		terrain_material.set_shader_parameter("ground_rotation_scale", 1.0 if tilemap_rotation else 0.0)
	
		terrain_mesh.resource_name = "Terrain"
		terrain_mesh.surface_set_material(0, terrain_material)
		
		var terrain := MeshInstance3D.new()
		terrain.name = "Terrain"
		terrain.mesh = terrain_mesh
		terrain.scale = Vector3.ONE * scale
		
		mission.add_child(terrain)
		terrain.owner = mission
		
		var hbx := BBHitbox.new()
		hbx.name = "Hitbox"
		hbx.collision_layer = 3
		hbx.collision_mask = 0
		
		terrain.add_child(hbx)
		hbx.owner = mission
		
		var terrain_heightmap := HeightMapShape3D.new()
		terrain_heightmap.map_width = size.x
		terrain_heightmap.map_depth = size.y
		terrain_heightmap.map_data = terrain_heights
		
		var hbx_shape := BBHitboxShape.new()
		hbx_shape.name = "0"
		hbx_shape.shape = terrain_heightmap
		hbx_shape.quads_data = quads_data
		
		hbx.add_child(hbx_shape)
		hbx_shape.owner = mission
		
		hbx.set_debug_color(Color.DARK_GREEN)
	
	var water_mesh := _heightmap_generate_mesh(size, water_heights)
	if water_mesh:
		var water_material := ShaderMaterial.new()
		water_material.resource_name = "WaterMaterial"
		water_material.shader = load("res://addons/bbtools/mission/shaders/water.gdshader") as Shader
		# Water Parameters
		water_material.set_shader_parameter("bump_texture", load("res://proprietary/loc/effects/water_bump.dds") as Texture2DArray)
		water_material.set_shader_parameter("fresnel_texture", load("res://proprietary/loc/textures/FRESNEL.dds") as Texture2D)
		
		# Sky Reflection Parameters
		_set_cloud_mat_params(water_material, sky_mode, sky_scale, sky_texture_paths)
		
		water_mesh.resource_name = "Water"
		water_mesh.surface_set_material(0, water_material)
		
		var water := MeshInstance3D.new()
		water.name = "Water"
		water.mesh = water_mesh
		water.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		water.scale = Vector3.ONE * scale
		
		mission.add_child(water)
		water.owner = mission
		mission.water = water

func _set_cloud_mat_params(clouds: ShaderMaterial, mode: int, scale: Vector2, sky_texture_paths: PackedStringArray) -> void:
	clouds.set_shader_parameter("sky_mode", mode)
	clouds.set_shader_parameter("sky_scale", scale)
	
	for i in sky_texture_paths.size():
		clouds.set_shader_parameter("sky%d_texture" % i, load(sky_texture_paths[i]) as Texture2D)

func _heightmap_generate_mesh(size: Vector2i, data: PackedFloat32Array) -> ArrayMesh:
	var verts: PackedVector3Array
	var uvs: PackedVector2Array
	
	var hmSize := size - Vector2i.ONE
	var uvSize := Vector2(hmSize)
	
	var offset := Vector3(hmSize.x, 0.0, hmSize.y) * 0.5
	
	for x : int in hmSize.x:
		for y : int in hmSize.y:
			var i : int = y * size.x + x
			
			# Tri 1
			if is_finite(data[i] + data[i + 1] + data[i + size.x]):
				verts.push_back(Vector3(x, data[i], y) - offset)
				uvs.push_back(Vector2(x, y) / uvSize)
				
				verts.push_back(Vector3(x + 1, data[i + 1], y) - offset)
				uvs.push_back(Vector2(x + 1, y) / uvSize)
				
				verts.push_back(Vector3(x, data[i + size.x], y + 1) - offset)
				uvs.push_back(Vector2(x, y + 1) / uvSize)
			
			# Tri 2
			if is_finite(data[i + size.x] + data[i + 1] + data[i + size.x + 1]):
				verts.push_back(Vector3(x, data[i + size.x], y + 1) - offset)
				uvs.push_back(Vector2(x, y + 1) / uvSize)
				
				verts.push_back(Vector3(x + 1, data[i + 1], y) - offset)
				uvs.push_back(Vector2(x + 1, y) / uvSize)
				
				verts.push_back(Vector3(x + 1, data[i + size.x + 1], y + 1) - offset)
				uvs.push_back(Vector2(x + 1, y + 1) / uvSize)
	
	if verts.is_empty():
		return null
	
	# Setup ArrayMesh
	var arrays: Array
	arrays.resize(ArrayMesh.ARRAY_MAX)
	
	arrays[ArrayMesh.ARRAY_VERTEX] = verts
	arrays[ArrayMesh.ARRAY_TEX_UV] = uvs
	
	# Generate normals and tangents
	var st := SurfaceTool.new()
	st.create_from_arrays(arrays, Mesh.PRIMITIVE_TRIANGLES)
	st.generate_normals()
	st.generate_tangents()
	return st.commit()
