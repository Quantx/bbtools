class_name BBSkeletonModifierDials extends SkeletonModifier3D

@export var dials: Array[BBDial] = [null, null, null, null, null, null] # 6
@export var max_speed_kmh: float

func _process_modification_with_delta(delta: float) -> void:
	var skeleton := get_skeleton()
	if !skeleton:
		return
	
	for dial in dials:
		if dial:
			dial.process(skeleton, delta)

func _validate_bone_names() -> void:
	var skeleton := get_skeleton()
	if !skeleton:
		return
	
	for dial in dials:
		if dial:
			dial.update_bone_idx(skeleton)

func set_dial_value(dial_type: BBDial.Type, value: float) -> void:
	if dials[dial_type]:
		dials[dial_type].value = clampf(value, 0.0, 1.0)

func set_speed_dial(speed_kmh: float) -> void:
	set_dial_value(BBDial.Type.Speed, speed_kmh / max_speed_kmh)
