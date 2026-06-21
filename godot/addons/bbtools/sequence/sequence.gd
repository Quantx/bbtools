class_name BBAnimationSequence extends Node

signal flags_event(flags : int)

@export var skeleton: Skeleton3D
@export var raycast_excludes: Array[CollisionObject3D]

const expected_surface_effects_size := BBHitbox.Surface.Max * 9
@export var surface_effects: PackedInt32Array

var _effects_one_shot : bool
var _sounds_one_shot : bool
var _flags_one_shot : bool

func _effects_callback(effect_args_list: Array) -> void:
	if _effects_one_shot:
		return
	_effects_one_shot = true
	
	for effect_args: Dictionary in effect_args_list:
		_spawn_effect(effect_args)

func _sounds_callback(sound_args_list: Array) -> void:
	if _sounds_one_shot:
		return
	_sounds_one_shot = true
	
	for sound_args: Dictionary in sound_args_list:
		pass

func _flags_callback(flags: int) -> void:
	if _flags_one_shot:
		return
	_flags_one_shot = true
	
	flags_event.emit(flags)

# There's a "bug" in the animation mixer that causes Method Tracks to be called multiple times due to blending
# The "mixer_applied" signal is fired after all the Method Tracks are called and can be used to reset the one shots
func on_mixer_applied() -> void:
	_effects_one_shot = false
	_sounds_one_shot = false
	_flags_one_shot = false

func _spawn_effect(args: Dictionary) -> void:
	var bone_idx := skeleton.find_bone(args.bone)
	assert(bone_idx >= 0)
	
	var attach_path := NodePath("%s:%s" % [skeleton.get_path(), args.bone])
	args.attach_path = attach_path
	
	var effect_config := args.get("effect", null) as BBEffectConfigGroup
	
	if "surface_idx" in args:
		# Get surface type from raycast
		var attach_transform := skeleton.global_transform * skeleton.get_bone_global_pose(bone_idx)
		var attach_position := attach_transform * (args.position as Vector3)
		
		var from := attach_position - Vector3(0, 10, 0)
		var to := attach_position + Vector3(0, 10, 0)
		
		var exclude: Array[RID]
		for n in raycast_excludes:
			exclude.append(n.get_rid())
		
		var mask : int = 0x1 | 0x10 # Terrain and World Physics
		var query := PhysicsRayQueryParameters3D.create(from, to, mask, exclude)
		
		var space_state := skeleton.get_world_3d().direct_space_state
		var ray := space_state.intersect_ray(query)
		var surface := BBHitbox.surface_from_ray(ray)
		if surface != BBHitbox.Surface.None:
			effect_config = BBEffectManager.load_surface(args.surface_idx as int, surface, surface_effects)
	
	if effect_config:
		BBEffectManager.spawn(effect_config, args, self)
