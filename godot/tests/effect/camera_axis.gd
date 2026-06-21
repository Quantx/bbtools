extends Node3D

@onready var camera := $Camera3D

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta: float) -> void:
	if Input.is_key_pressed(KEY_LEFT):
		global_rotate(Vector3.DOWN, delta)
	if Input.is_key_pressed(KEY_RIGHT):
		global_rotate(Vector3.UP, delta)
		
	if Input.is_key_pressed(KEY_UP):
		rotate_object_local(Vector3.LEFT, delta)
	if Input.is_key_pressed(KEY_DOWN):
		rotate_object_local(Vector3.RIGHT, delta)
	
	if Input.is_key_pressed(KEY_PAGEUP):
		camera.translate_object_local(Vector3.FORWARD * 100.0 * delta)
	if Input.is_key_pressed(KEY_PAGEDOWN):
		camera.translate_object_local(Vector3.BACK * 100.0 * delta)
	
	if camera.position.z < 10.0:
		camera.position.z = 10.0
