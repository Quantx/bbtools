class_name BBEffectConfig extends Resource

enum EffectType {
	Sin2D,
	Rep2D,
	Par2D,
	ParLin,
	GK, # Unused
	Sin3D,
	Rep3D,
	Par3D,
}

enum BlendType {
	Alpha,
	Add,
	Sub,
}

enum Flags {
	RevVelocity = 0x1,
	NoFog = 0x2,
	DirSprite = 0x4,
	NoBillboard = 0x100,
	Cockpit = 0x8000,
}

class EffectTransform extends Resource:
	@export var position: Vector3
	@export var rotation: Vector3
	@export var scale: Vector3
	
	func _init(file: FileAccess) -> void:
		var vec3s := file.get_buffer(36).to_vector3_array()
		position = vec3s[0]
		rotation = vec3s[1]
		scale = vec3s[2]

func is_2D() -> bool:
	return type <= EffectType.ParLin

func is_3D() -> bool:
	return type >= EffectType.Sin3D

func is_repeat() -> bool:
	return type in [EffectType.Rep2D, EffectType.Par2D, EffectType.Rep3D, EffectType.Par3D]

@export var type: EffectType

# Only used by 2D Effects
@export var spritesheet: BBSpriteSheet
@export var sequence: BBEffectSequence

# Only used by 3D Effects
@export_file("*.gltf") var model_path: String
@export var model: Mesh:
	get = _get_model

@export var blend: BlendType

@export var life: float
@export var delay: float

@export var priority: int
@export var flags: int

@export var child_effect: BBEffectConfigGroup
@export var child_effect_interval: float

@export var vertex_color: Color
@export var damping_color: Color
@export var damping_delay: float

@export var initial: EffectTransform
@export var velocity: EffectTransform
@export var acceleration: EffectTransform

@export var gravity_acceleration: Vector3

#region repeat_data
@export var repeat_count: int = 1
@export var repeat_count_random: int
@export var repeat_interval: float

@export var vertex_color_random: Color

@export var life_random: float

@export var initial_position_random: Vector3
@export var initial_scale_xy_random: float
@export var initial_rotation_z_random: float

@export var velocity_position_rotation_random: Vector3
@export var velocity_position_offset_random: Vector3
#endregion

func _get_model() -> Mesh:
	if !model && model_path:
		var scene := load(model_path) as PackedScene
		var root := scene.instantiate()
		var mesh_inst := root.get_node(^"Skeleton3D/0") as MeshInstance3D
		model = mesh_inst.mesh
		root.free()
	return model

func get_common_resource() -> Resource:
	if is_2D():
		return spritesheet.texture
	elif is_3D():
		return model
	return null

func get_common_name() -> String:
	var resource := get_common_resource()
	if !resource.resource_name.is_empty():
		return resource.resource_name
	return resource.resource_path.get_file().get_slice(".", 0)

func _rri(range: int) -> int:
	return randi_range(-range, range)

func _rrf(range: float) -> float:
	return randf_range(-range, range)

func instantiate() -> Array[BBEffect]:
	var eff_list: Array[BBEffect]
	
	var eff_count := repeat_count
	if type == EffectType.Par2D || type == EffectType.Par3D:
		eff_count = maxi(eff_count + _rri(repeat_count_random), 1)
	
	for eff_index in eff_count:
		var eff := BBEffect.new()
		eff.config = self
		
		eff.timer = -(0.001 + delay + repeat_interval * eff_index)
		
		eff.vertex_color.r8 = posmod(vertex_color.r8 + _rri(vertex_color_random.r8), 0x100)
		eff.vertex_color.g8 = posmod(vertex_color.g8 + _rri(vertex_color_random.g8), 0x100)
		eff.vertex_color.b8 = posmod(vertex_color.b8 + _rri(vertex_color_random.b8), 0x100)
		eff.vertex_color.a8 = posmod(vertex_color.a8 + _rri(vertex_color_random.a8), 0x100)
		
		eff.life = life + _rrf(life_random)
		
		eff.initial_position = initial.position
		eff.initial_rotation = initial.rotation
		eff.initial_scale = initial.scale
		
		eff.initial_position.x += _rrf(initial_position_random.x)
		eff.initial_position.y += _rrf(initial_position_random.y)
		eff.initial_position.z += _rrf(initial_position_random.z)
		
		eff.initial_scale.x += _rrf(initial_scale_xy_random)
		eff.initial_scale.y += _rrf(initial_scale_xy_random)
		
		eff.initial_rotation.z += _rrf(initial_rotation_z_random)
		
		var velocity_position_rotation: Vector3
		velocity_position_rotation.x = _rrf(velocity_position_rotation_random.x)
		velocity_position_rotation.y = _rrf(velocity_position_rotation_random.y)
		velocity_position_rotation.z = _rrf(velocity_position_rotation_random.z)
		
		eff.velocity_position = velocity.position * Quaternion.from_euler(velocity_position_rotation)
		
		eff.velocity_position.x += _rrf(velocity_position_offset_random.x)
		eff.velocity_position.y += _rrf(velocity_position_offset_random.y)
		eff.velocity_position.z += _rrf(velocity_position_offset_random.z)
		
		eff.acceleration_position = acceleration.position
		
		eff.velocity_direction_last = eff.velocity_position.sign()
		
		eff_list.append(eff)
	
	return eff_list
