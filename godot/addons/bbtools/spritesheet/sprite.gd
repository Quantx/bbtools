class_name BBSpriteSheet extends Resource

class Frame extends Resource:
	@export var region: Rect2
	@export var scale: Vector2
	
	func _init(start: Vector2, end: Vector2, _scale: Vector2) -> void:
		region.position = start
		region.end = end
		scale = _scale
	
	func image_region(image_size: Vector2) -> Rect2:
		var image_region := region
		image_region.position *= image_size
		image_region.size *= image_size
		return image_region

# Cockpit Display Textures are 3D
@export var texture: Texture
@export var offset: Vector2
@export var frames: Array[Frame]

func get_size() -> Vector2:
	if texture is Texture2D:
		return (texture as Texture2D).get_size()
	
	if texture is Texture3D:
		var tex_3D := texture as Texture3D
		return Vector2(tex_3D.get_width(), tex_3D.get_height())
	
	push_error("Unknown texture format for BBSprite: ", texture)
	return Vector2.ZERO

func frame_to_texture(frame: Frame) -> ImageTexture:
	if texture is not Texture2D:
		return null
	
	var src_image := (texture as Texture2D).get_image()
	if src_image.is_compressed():
		var err := src_image.decompress()
		if err != OK:
			print("Failed to decompress src_image: ", err)
	
	var image_size := Vector2(src_image.get_size())
	var region := Rect2i(frame.region.position * image_size, frame.region.size * image_size)
	
	# WARNING: Do not modify src_image as it will affect the underlying texture
	var dst_image := Image.create_empty(region.size.x, region.size.y, false, src_image.get_format())
	dst_image.blit_rect(src_image, region, Vector2i.ZERO)
	
	return ImageTexture.create_from_image(dst_image)
