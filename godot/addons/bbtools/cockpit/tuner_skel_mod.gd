class_name BBSkeletonModifierTuner extends SkeletonModifier3D

@export var cockpit: BBCockpit

@export var travel: Vector3
@export var speed: float = 5.0
var _index_current: float

@export var bone: String
var _bone_idx: int = -1

func _process_modification_with_delta(delta: float) -> void:
	var skeleton := get_skeleton()
	if !skeleton || !cockpit:
		return
	
	_index_current = move_toward(_index_current, clampi(cockpit.tuner_index_target, 0, 4), speed * delta)
	
	var pose := skeleton.get_bone_pose(_bone_idx)
	pose = pose.translated_local(travel * _index_current)
	skeleton.set_bone_pose(_bone_idx, pose)

func _validate_bone_names() -> void:
	var skeleton := get_skeleton()
	if !skeleton:
		return
	
	_bone_idx = skeleton.find_bone(bone)
