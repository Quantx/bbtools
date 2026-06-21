class_name BBPointLight extends OmniLight3D

@export var life: float
@export var duration: float

@export var end_color: Color
@export var end_size: float

@export var autostart: bool = true
@export var oneshot: bool = true

var _start_color: Color
var _start_size: float

var _tween: Tween
func _ready() -> void:
	_start_color = light_color
	_start_size = omni_range
	
	if autostart:
		start()
	else:
		hide()

func start() -> void:
	if _tween && _tween.is_running():
		_tween.kill()
	
	show()
	light_color = _start_color
	omni_range = _start_size
	
	_tween = create_tween()
	
	if duration > 0.0:
		_tween.tween_property(self, "light_color", end_color, duration)
		_tween.tween_property(self, "omni_range", end_size, duration)
	
	if oneshot:
		_tween.tween_callback(queue_free).set_delay(life)
	else:
		_tween.tween_callback(hide).set_delay(life)

func stop() -> void:
	if _tween && _tween.is_running():
		_tween.kill()
	
	hide()
