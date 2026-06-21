@abstract class_name BBBootAnimBase extends Resource

@export var time: float
@export var duration: float

func is_active(now: float) -> bool:
	return now >= time

func get_weight(now: float) -> float:
	return minf((now - time) / duration, 1.0)
