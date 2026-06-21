@tool
class_name EditorSceneFormatImporterBBProjectile extends EditorSceneFormatImporter

func _get_extensions() -> PackedStringArray:
	return ["projectile_scene"]

func _get_import_flags() -> int:
	return IMPORT_SCENE

func _import_scene(path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	
	var projectile := BBProjectile.new()
	projectile.name = "Projectile"
	
	var texture := load("res://proprietary/loc/textures/SCIDOBJ.dds") as Texture2D
	
	var material := ShaderMaterial.new()
	material.shader = load("res://addons/bbtools/weapon/projectile_scene/projectile.gdshader") as Shader
	material.set_shader_parameter("albedo_texture", texture)
	projectile.material = material
	
	var id := file.get_8()
	var _type := file.get_8()
	
	var config_path := base_path.path_join("%02d.weapon" % id)
	projectile.config = load(config_path) as BBWeaponConfig
	
	var model_scene_path := file.get_pascal_string()
	var model_scene := load(model_scene_path) as PackedScene
	var model := model_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
	
	#var model := Node3D.new()
	#model.scene_file_path = file.get_pascal_string()
	
	model.name = "Model"
	
	projectile.add_child(model)
	model.owner = projectile
	projectile.model = model
	
	var collider_type := file.get_8()
	match collider_type:
		0:
			projectile.position = Vector3(file.get_float(), file.get_float(), file.get_float())
			
			var sphere_shape := SphereShape3D.new()
			sphere_shape.radius = file.get_float()
			projectile.shape = sphere_shape
		1:
			projectile.position = Vector3(file.get_float(), file.get_float(), file.get_float())
			projectile.quaternion = Quaternion(file.get_float(), file.get_float(), file.get_float(), file.get_float())
			
			var capsule_shape := CapsuleShape3D.new()
			capsule_shape.radius = file.get_float()
			capsule_shape.height = file.get_float()
			projectile.shape = capsule_shape
		2:
			projectile.position = Vector3(file.get_float(), file.get_float(), file.get_float())
			projectile.quaternion = Quaternion(file.get_float(), file.get_float(), file.get_float(), file.get_float())
			
			var box_shape := BoxShape3D.new()
			box_shape.size = Vector3(file.get_float(), file.get_float(), file.get_float())
			projectile.shape = box_shape
		_:
			push_error("Unknown collider shape: %d" % collider_type)
	
	model.transform = projectile.transform.inverse()
	
	var flare_effect_path := file.get_pascal_string()
	if !flare_effect_path.is_empty():
		projectile.flare_effect = load(flare_effect_path) as BBEffectConfigGroup
	
	var flying_effect_count := file.get_32()
	for i in flying_effect_count:
		var flying_effect_path := file.get_pascal_string()
		var flying_effect := load(flying_effect_path) as BBEffectConfigGroup
		projectile.flying_effects.append(flying_effect)
	
	var thrust_effect_path := file.get_pascal_string()
	if !thrust_effect_path.is_empty():
		projectile.thrust_effect = load(thrust_effect_path) as BBEffectConfigGroup
	
	var mech_impact_effect_path := file.get_pascal_string()
	if !mech_impact_effect_path.is_empty():
		projectile.mech_impact_effect = load(mech_impact_effect_path) as BBEffectConfigGroup
	
	var smoke_trail_path := file.get_pascal_string()
	if !smoke_trail_path.is_empty():
		var smoke_trail_scene := load(smoke_trail_path) as PackedScene
		var smoke_trail := smoke_trail_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as BBRibbonTrail
		#var smoke_trail := BBRibbonTrail.new()
		#smoke_trail.scene_file_path = smoke_trail_path
		smoke_trail.name = "SmokeTrail"
		
		projectile.add_child(smoke_trail)
		smoke_trail.owner = projectile
		smoke_trail.following = projectile
	
	var tracer_trail_path := file.get_pascal_string()
	if !tracer_trail_path.is_empty():
		var tracer_trail_scene := load(tracer_trail_path) as PackedScene
		var tracer_trail := tracer_trail_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as BBRibbonTrail
		#var tracer_trail := BBRibbonTrail.new()
		#tracer_trail.scene_file_path = tracer_trail_path
		tracer_trail.name = "TracerTrail"
		
		projectile.add_child(tracer_trail)
		tracer_trail.owner = projectile
		tracer_trail.following = projectile
	
	return projectile
