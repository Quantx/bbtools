class_name BBDial extends Resource

enum Type {
	FuelMain,
	Speed,
	Battery,
	RPM,
	FuelSub1,
	FuelSub2,
}

@export var bone: String
var _bone_idx: int = -1

@export var scale: float
@export var speed: float = 2.0

var value: float
var _angle: float

func update_bone_idx(skeleton: Skeleton3D) -> void:
	_bone_idx = skeleton.find_bone(bone)

func process(skeleton: Skeleton3D, delta: float) -> void:
	_angle = move_toward(_angle, value * scale, delta * speed)
	
	var pose := skeleton.get_bone_pose(_bone_idx)
	pose = pose.rotated_local(Vector3.FORWARD, _angle)
	skeleton.set_bone_pose(_bone_idx, pose)
