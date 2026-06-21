class_name BBHitboxQuadsData extends Resource

@export var layers : PackedByteArray
@export var surfaces : PackedByteArray

func get_layer(quad_index : int) -> int:
	return layers.decode_u16(quad_index * 2)

func get_surface(quad_index : int) -> BBHitbox.Surface:
	return surfaces[quad_index]
