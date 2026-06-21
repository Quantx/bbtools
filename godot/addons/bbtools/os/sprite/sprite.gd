class_name BBSprite extends Resource

@export var spritesheet: BBSpriteSheet
@export var frame_idx: int

@export var position: Vector2
@export var origin: Vector2
@export var size: Vector2
@export var rotation: float
@export var scale: float = 1.0

@export var pallete_idx: int = -1
@export var color: Color

func get_frame() -> BBSpriteSheet.Frame:
	return spritesheet.frames[frame_idx]
