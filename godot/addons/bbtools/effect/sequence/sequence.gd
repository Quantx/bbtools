class_name BBEffectSequence extends Resource

enum FrameType {
	Delay,
	Exit,
	Reset,
	Pause,
}

class Frame extends Resource:
	@export var type: FrameType
	@export var index: int
	@export var delay: float # Only used by FrameType.Delay

@export var frames: Array[Frame]
