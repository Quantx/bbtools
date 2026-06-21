class_name BBRibbonTrail extends Skeleton3D

@export var following: Node3D

@export var sample_interval: float

@export var flags: int

var _points: Array[Transform3D]

var _sample_timer: float

func _process(delta: float) -> void:
	if !following:
		return
	
	var follow_trans := following.get_global_transform_interpolated()
	if _points.is_empty():
		_points.push_front(follow_trans)
	
	_sample_timer += delta
	if _sample_timer >= sample_interval:
		_sample_timer -= sample_interval
		
		_points.push_front(follow_trans)
		if _points.size() >= get_bone_count():
			_points.pop_back()
	
	set_bone_pose(0, follow_trans.rotated_local(Vector3.LEFT, PI * 0.5))
	for bone_idx in range(1, get_bone_count()):
		var i := bone_idx - 1
		
		var bone_trans: Transform3D
		if i < _points.size():
			bone_trans = _points[i].rotated_local(Vector3.LEFT, PI * 0.5)
		else:
			bone_trans = Transform3D(Basis.from_scale(Vector3.ONE * 0.001), _points[-1].origin)
		
		set_bone_pose(bone_idx, bone_trans)

func reset() -> void:
	_points.clear()
	_sample_timer = 0.0
	for i : int in get_bone_count():
		set_bone_pose(i, Transform3D.IDENTITY)
