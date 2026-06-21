class_name BBMission extends Node3D

@export_enum("Day", "Evening", "Night", "Campaign") var current_stage: int:
	set = _set_current_stage
@export var stages: Array[BBStage]

@export var map: Texture2D

@export var sun: DirectionalLight3D
@export var water: MeshInstance3D
@export var environment: Environment

@export var water_heights: PackedFloat32Array

@export var object_material: Material
func _ready() -> void:
	if object_material:
		# Apply object material if one was specified
		for n in $Objects.get_children():
			var mesh_inst := n.get_node(^"Skeleton3D/0") as MeshInstance3D
			mesh_inst.material_override = object_material

func _set_current_stage(new_stage: int) -> void:
	if stages.is_empty():
		current_stage = -1
		return
	
	current_stage = clampi(new_stage, 0, stages.size())
	
	var stage := stages[current_stage]
	
	sun.light_color = stage.world_light
	sun.light_energy = stage.world_light.a
	sun.visible = !stage.world_light.is_equal_approx(Color.BLACK)
	sun.rotation = Vector3(-stage.shadow_pitch, stage.shadow_yaw, 0.0)
	sun.shadow_enabled = stage.draw_shadows
	sun.directional_shadow_max_distance = stage.shadow_end
	
	environment.ambient_light_color = stage.world_ambient
	environment.ambient_light_energy = 0.0 if stage.world_ambient.is_equal_approx(Color.BLACK) else stage.world_ambient.a
	
	environment.fog_light_color = stage.fog_color
	environment.fog_light_energy = stage.fog_color.a
	environment.fog_depth_begin = stage.fog_start
	environment.fog_depth_end = stage.fog_end
	
	if environment.sky:
		var sky_material := environment.sky.sky_material as ShaderMaterial
		sky_material.shader
		sky_material.set_shader_parameter("fog_color", stage.fog_color)
		_set_cloud_mat_params(sky_material, stage)
	
	if water:
		var water_mesh := water.mesh as ArrayMesh
		var water_material := water_mesh.surface_get_material(0) as ShaderMaterial
		
		# Water Parameters
		water_material.set_shader_parameter("water_color", stage.water_color)
		
		# Sky Reflection Parameters
		_set_cloud_mat_params(water_material, stage)

func _set_cloud_mat_params(clouds: ShaderMaterial, stage: BBStage) -> void:
	clouds.set_shader_parameter("sky_height", stage.sky_height)
	clouds.set_shader_parameter("sky0_velocity", stage.sky_velocity_0)
	clouds.set_shader_parameter("sky1_velocity", stage.sky_velocity_1)
	clouds.set_shader_parameter("fog_start", stage.sky_fog_start)
	clouds.set_shader_parameter("fog_end", stage.sky_fog_end)

func get_water_height() -> float:
	if !water || !water.visible:
		return -INF
	
	return water.global_position.y
