@tool
class_name EditorSceneFormatImporterBBRibbonTrail extends EditorSceneFormatImporter

func _get_extensions() -> PackedStringArray:
	return ["trail"]

func _get_import_flags() -> int:
	return IMPORT_SCENE

func _import_scene(path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	
	var trail := BBRibbonTrail.new()
	trail.name = "RibbonTrail"
	trail.top_level = true
	
	var sprite_path := file.get_pascal_string()
	var sprite := load(sprite_path) as BBSpriteSheet
	var texture := sprite.frame_to_texture(sprite.frames[0])
	
	var color_start := Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
	var color_max := Color.from_rgba8(file.get_8(), file.get_8(), file.get_8(), file.get_8())
	
	var gradient := Gradient.new()
	gradient.colors = []
	gradient.offsets = []
	
	gradient.add_point(0.0, color_start)
	gradient.add_point(2.0 / 3.0, color_max)
	gradient.add_point(1.0, Color(0.0, 0.0, 0.0, 0.0))
	
	var gradient_texture := GradientTexture1D.new()
	gradient_texture.gradient = gradient
	
	var section_count := file.get_8()
	var texture_scale := file.get_float()
	
	var width_start := file.get_float()
	var width_end := file.get_float()
	
	var curve := Curve.new()
	curve.add_point(Vector2(1.0, width_start), 0, 0, Curve.TANGENT_LINEAR, Curve.TANGENT_LINEAR)
	curve.add_point(Vector2(0.0, width_end), 0, 0, Curve.TANGENT_LINEAR, Curve.TANGENT_LINEAR)
	
	trail.sample_interval = file.get_float()
	
	trail.flags = file.get_32()
	
	var material := ShaderMaterial.new()
	material.resource_name = "TrailMaterial"
	material.shader = load("res://addons/bbtools/trail/trail.gdshader") as Shader
	material.set_shader_parameter("albedo_texture", texture as Texture2D)
	material.set_shader_parameter("albedo_texture_repeat", texture_scale)
	material.set_shader_parameter("albedo_gradient", gradient_texture)
	
	var ribbon := RibbonTrailMesh.new()
	ribbon.resource_name = "Trail"
	ribbon.curve = curve
	ribbon.section_length = 1.0
	ribbon.section_segments = 1
	ribbon.sections = section_count
	ribbon.surface_set_material(0, material)
	
	var mesh_inst := MeshInstance3D.new()
	mesh_inst.name = "0"
	mesh_inst.mesh = ribbon
	
	trail.add_child(mesh_inst)
	mesh_inst.owner = trail
	
	# Setup Skeleton
	var depth := ribbon.sections * ribbon.section_length
	for i in ribbon.sections + 1:
		assert(trail.add_bone("%d" % i) == i)
		
		var v := float(i) / ribbon.sections
		
		var y := depth * v
		y = (depth * 0.5) - y
		
		trail.set_bone_rest(i, Transform3D(Basis.IDENTITY, Vector3.UP * y))
	
	mesh_inst.skeleton = ^".."
	mesh_inst.skin = trail.create_skin_from_rest_transforms()
	trail.register_skin(mesh_inst.skin)
	
	return trail
