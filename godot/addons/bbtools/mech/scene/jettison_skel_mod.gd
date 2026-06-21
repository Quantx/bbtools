class_name BBSkeletonModiferJettison extends SkeletonModifier3D

enum JettisonBoneState {
	Start,
	Run,
	Stop
}

class JettisonBone extends RefCounted:
	static var _gravity: Vector3
	static func _static_init() -> void:
		var _gravity_vector := ProjectSettings.get_setting("physics/3d/default_gravity_vector") as Vector3
		var _gravity_force := ProjectSettings.get_setting("physics/3d/default_gravity") as float
		_gravity = _gravity_vector * _gravity_force
	
	var bone_idx: int
	var state: JettisonBoneState
	
	var velocity: Vector3
	var local_velocity: Vector3
	var transform: Transform3D
	
	func _init(_bone_idx: int, _local_velocity: Vector3) -> void:
		bone_idx = _bone_idx
		local_velocity = _local_velocity
		
		state = JettisonBoneState.Start
		transform = Transform3D.IDENTITY
	
	func process(delta: float) -> void:
		if state != JettisonBoneState.Run:
			return
		
		velocity += _gravity * delta
		transform = transform.translated(velocity * delta).translated_local(local_velocity * delta)
		
		if transform.origin.y < -1000.0:
			transform = transform.scaled_local(Vector3.ONE * 0.001) # Shrink the model to hide it
			state = JettisonBoneState.Stop
	
	func process_modification(skeleton: Skeleton3D) -> void:
		if state == JettisonBoneState.Start:
			state = JettisonBoneState.Run
			transform = skeleton.get_bone_global_pose(bone_idx)
		
		skeleton.set_bone_global_pose(bone_idx, transform)

var _jettisoned_bones: Dictionary[StringName,JettisonBone]

func _process(delta: float) -> void:
	for bs: JettisonBone in _jettisoned_bones.values():
		bs.process(delta)

func _process_modification() -> void:
	var skeleton := get_skeleton()
	if !skeleton:
		return
	
	for bs: JettisonBone in _jettisoned_bones.values():
		bs.process_modification(skeleton)

func jettison_bone(bone_name: StringName, local_velocity: Vector3) -> void:
	if bone_name in _jettisoned_bones:
		return
	
	var skeleton := get_skeleton()
	if !skeleton:
		return
	
	var bone_idx := skeleton.find_bone(bone_name)
	if bone_idx < 0:
		return
	
	_jettisoned_bones[bone_name] = JettisonBone.new(bone_idx, local_velocity)

func reset_bone(bone_name: StringName) -> void:
	_jettisoned_bones.erase(bone_name)

func reset_all_bones() -> void:
	_jettisoned_bones.clear()
