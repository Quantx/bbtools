class_name BBEffect extends Object

static func _int_to_float(value: int) -> float:
	var buf: PackedByteArray
	buf.resize(4)
	
	buf.encode_u32(0, value)
	return buf.decode_float(0)

static func _float_to_int(value: float) -> int:
	var buf: PackedByteArray
	buf.resize(4)
	
	buf.encode_float(0, value)
	return buf.decode_u32(0)

var config: BBEffectConfig

## How long since this effect was created, will often be negative which indicates an initial delay
var timer: float

## How long this effect should live for
var life: float

var vertex_color: Color

var initial_position: Vector3
var initial_rotation: Vector3
var initial_scale: Vector3

var velocity_position: Vector3
var acceleration_position: Vector3

# How long until the next child effect should be spawned
var child_effect_timer: float
var child_effect_spawn_func: Callable

var velocity_direction_last: Vector3
var reverse_velocity_time: float = -1.0

var sequence_index: int
var sequence_timer: float

const buffer_size := 20
var buffer: PackedFloat32Array

var enabled: bool = true

var attach_node: Node3D
var attach_transform := Transform3D.IDENTITY
var attach_bone_idx: int
var attach_bone_transform := Transform3D.IDENTITY

var detach_delay: float = -1.0

var is_owned: bool = false
var owner: Node

func on_attached_skeleton_updated(skeleton: Skeleton3D) -> void:
	if attach_node:
		attach_bone_transform = skeleton.get_bone_global_pose(attach_bone_idx)
	else:
		skeleton.skeleton_updated.disconnect(on_attached_skeleton_updated)

func update(delta: float) -> void:
	if !enabled:
		return
	
	timer += delta
	
	if timer < 0.0 || is_done():
		return
	
	var timer_position := timer
	
	if (config.flags & BBEffectConfig.Flags.RevVelocity) == BBEffectConfig.Flags.RevVelocity:
		var velocity_direction := (velocity_position + acceleration_position * timer).sign()
		
		if !velocity_direction.is_equal_approx(velocity_direction_last):
			reverse_velocity_time = timer
		
		velocity_direction_last = velocity_direction
		
		if reverse_velocity_time >= 0.0:
			timer_position = reverse_velocity_time
	
	var position := initial_position + (velocity_position + (acceleration_position + config.gravity_acceleration) * timer_position * 0.5) * timer
	var rotation := initial_rotation + (config.velocity.rotation + (config.acceleration.rotation * timer)) * timer
	
	var effect_transform := Transform3D(Quaternion.from_euler(rotation), position)
	
	var uv_data := Vector2.ZERO
	if config.is_2D():
		var sequence_frame := get_sequence_frame()
		if sequence_frame.type == BBEffectSequence.FrameType.Delay:
			sequence_timer += delta
			if sequence_timer >= sequence_frame.delay:
				sequence_timer -= sequence_frame.delay
				
				sequence_index += 1
				if get_sequence_frame().type == BBEffectSequence.FrameType.Reset:
					sequence_index = 0
		
		var sprite_frame := get_sprite_frame()
		var uv_offset := sprite_frame.region.position
		var uv_scale := sprite_frame.region.size
		
		assert(uv_scale.x < 1.0 && uv_scale.y < 1.0)
		
		uv_data = (uv_offset * config.spritesheet.get_size()).floor() + uv_scale
		
		var scale := initial_scale + (config.velocity.scale + (config.acceleration.scale * timer)) * timer
		
		effect_transform = effect_transform.scaled_local(scale)
		
		if timer > config.damping_delay:
			var delta_damping_color := config.damping_color
			delta_damping_color.r **= delta
			delta_damping_color.g **= delta
			delta_damping_color.b **= delta
			delta_damping_color.a **= delta
			
			vertex_color = (vertex_color * delta_damping_color).clamp()
	
	if is_instance_valid(attach_node):
		if attach_node.is_inside_tree():
			attach_transform = attach_node.global_transform
		
		if detach_delay >= 0.0 && timer > detach_delay:
			attach_node = null
	
	var transform := attach_transform * attach_bone_transform * effect_transform
	
	if config.child_effect:
		child_effect_timer += delta
		if child_effect_timer >= config.child_effect_interval:
			child_effect_timer -= config.child_effect_interval
			
			child_effect_spawn_func.call(config.child_effect, {"position": transform.origin}, owner)
	
	# TRANSFORM_X
	buffer[0] = transform.basis.x.x
	buffer[1] = transform.basis.y.x
	buffer[2] = transform.basis.z.x
	buffer[3] = transform.origin.x
	
	# TRANSFORM_Y
	buffer[4] = transform.basis.x.y
	buffer[5] = transform.basis.y.y
	buffer[6] = transform.basis.z.y
	buffer[7] = transform.origin.y
	
	# TRANSFORM_Z
	buffer[8] = transform.basis.x.z
	buffer[9] = transform.basis.y.z
	buffer[10] = transform.basis.z.z
	buffer[11] = transform.origin.z
	
	# VERTEX_COLOR
	buffer[12] = vertex_color.r
	buffer[13] = vertex_color.g
	buffer[14] = vertex_color.b
	buffer[15] = vertex_color.a
	
	var flags: int = 0
	if (config.flags & BBEffectConfig.Flags.NoBillboard) != BBEffectConfig.Flags.NoBillboard:
		flags |= 1 << 0
	
	if config.blend != BBEffectConfig.BlendType.Alpha:
		flags |= 1 << 1
	
	# Can't disable fog this way: https://github.com/godotengine/godot/blob/master/servers/rendering/renderer_rd/shaders/forward_clustered/scene_forward_clustered.glsl#L1516
	if (config.flags & BBEffectConfig.Flags.NoFog) == BBEffectConfig.Flags.NoFog:
		flags |= 1 << 2
	
	if (config.flags & BBEffectConfig.Flags.Cockpit) == BBEffectConfig.Flags.Cockpit:
		flags |= 1 << 3
	
	var flags_f := _int_to_float(flags)
	assert(flags == _float_to_int(flags_f))
	
	# CUSTOM_DATA
	buffer[16] = uv_data.x
	buffer[17] = uv_data.y
	buffer[18] = flags_f
	#buffer[18] = 0.0 if (config.flags & BBEffectConfig.Flags.NoBillboard) == BBEffectConfig.Flags.NoBillboard else 1.0
	#buffer[19] = 1.0 if config.blend != BBEffectConfig.BlendType.Alpha else 0.0

func get_sequence_frame() -> BBEffectSequence.Frame:
	return config.sequence.frames[sequence_index]

func get_sprite_frame() -> BBSpriteSheet.Frame:
	var sequence_frame := get_sequence_frame()
	return config.spritesheet.frames[sequence_frame.index]

func is_visible() -> bool:
	return timer >= 0 && enabled

func is_done() -> bool:
	return timer >= life || (is_owned && !is_instance_valid(owner)) || (config.is_2D() && get_sequence_frame().type == BBEffectSequence.FrameType.Exit)
