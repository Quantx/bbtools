class_name BBBootSystems extends Resource

@export var system_count: int

@export var switch_error_color: Color
@export var switch_primary_color: Color

@export var switch_error_vertices: PackedVector2Array
func get_switch_error_rect(idx: int) -> Rect2:
	assert(idx <= system_count)
	
	idx *= 2
	var rect := Rect2(switch_error_vertices[idx], Vector2.ZERO)
	rect.end = switch_error_vertices[idx + 1]
	return rect

@export var switch_progress_quad_count: int # Quads per system
@export var switch_progress_vertices: PackedVector2Array
func get_switch_progress_quad(idx: int) -> PackedVector2Array:
	assert(idx <= system_count * switch_progress_quad_count)
	
	idx *= 4
	return switch_progress_vertices.slice(idx, idx + 4)

@export var switch_success_text: String
@export var switch_success_positions: PackedVector2Array

@export var startup_progress_count: int # Progress bars per system
@export var startup_progress_positions: PackedVector2Array
