class_name BBCockpitLighting extends Node3D

signal hatch_changed
var hatch_open: bool

@export var environment: Environment

func _hatch_callback(open: bool) -> void:
	hatch_open = open
	hatch_changed.emit()
