class_name BBSkeletonModiferChassis extends SkeletonModifier3D

var torso_rotation_y: float

@export var recoil_bone_name: StringName
@export var recoil_translation: Vector3
var recoil_weight: float
var _recoil_weight_current: float

func _process(delta: float) -> void:
	_recoil_weight_current = move_toward(_recoil_weight_current, recoil_weight, delta * 2.0)

func _process_modification() -> void:
	var skeleton := get_skeleton()
	if !skeleton:
		return
	
	var torso_bone_idx := skeleton.find_bone(BBMech.get_special(BBMech.Bones.Torso))
	if torso_bone_idx >= 0:
		skeleton.set_bone_pose_rotation(torso_bone_idx, Quaternion(Vector3.DOWN, torso_rotation_y))
	
	if recoil_bone_name:
		var bone_idx := skeleton.find_bone(recoil_bone_name)
		if bone_idx >= 0:
			var pose := skeleton.get_bone_pose(bone_idx)
			pose = pose.translated_local(recoil_translation * _recoil_weight_current)
			skeleton.set_bone_pose(bone_idx, pose)
