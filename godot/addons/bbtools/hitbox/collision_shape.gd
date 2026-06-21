class_name BBHitboxShape extends CollisionShape3D

@export var quads_data: BBHitboxQuadsData
@export var bone_name : String

func _get_quad_index(ray : Dictionary) -> int:
	if shape is HeightMapShape3D:
		var ray_position := to_local(ray.position)
		
		var heightmap := shape as HeightMapShape3D
		var hm_size := Vector2i(heightmap.map_width, heightmap.map_depth) - Vector2i.ONE
		var hm_pos := Vector2i(ray_position.x, ray_position.z) + (hm_size / 2)
		return hm_pos.y * heightmap.map_width + hm_pos.x
	
	assert(ray.face_index >= 0)
	return int(ray.face_index) / 2 # Conver from Triangles to Quads

func get_surface(ray : Dictionary) -> BBHitbox.Surface:
	return quads_data.get_surface(_get_quad_index(ray))

func get_layer(ray : Dictionary) -> int:
	return quads_data.get_layer(_get_quad_index(ray))
