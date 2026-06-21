@tool
class_name BBHitbox extends StaticBody3D

enum Layer {
	None = 0,
	
	layer1  = 0x0001,
	layer2  = 0x0002,
	layer3  = 0x0004,
	layer4  = 0x0008,
	layer5  = 0x0010,
	layer6  = 0x0020,
	layer7  = 0x0040,
	layer8  = 0x0080,
	layer9  = 0x0100,
	layer10 = 0x0200, 
	layer11 = 0x0400,
	layer12 = 0x0800,
	layer13 = 0x1000,
	layer14 = 0x2000,
	ShotHitbox = 0x4000,
	PhysHitbox = 0x8000,
}

enum Surface {
	surf_00		= 0x00,
	surf_01		= 0x01,
	Ground_02	= 0x02, # Airbase map (This is water on map 10)
	Ground_03	= 0x03, # Normal ground
	Ground_04	= 0x04, # Rocky / Swamp (Rough terrrain?)
	Road_05		= 0x05, # Used on underground maps
	surf_06		= 0x06,
	Concrete_07	= 0x07, # Used on battelship map
	surf_08		= 0x08,
	surf_09		= 0x09,
	surf_0A		= 0x0A,
	Water_0B	= 0x0B,
	UndergroundWall_0C = 0x0C,
	Metal_0D	= 0x0D,
	surf_0E		= 0x0E,
	surf_0F		= 0x0F,
	Water_10	= 0x10,
	Ground_11	= 0x11, # Mountain terrain
	surf_12		= 0x12,
	Tent_13		= 0x13,
	Torso_14	= 0x14,
	surf_15		= 0x15,
	surf_16		= 0x16,
	Hill_17		= 0x17,
	Forest_18	= 0x18,
	LegRight_19	= 0x19,
	LegLeft_1A	= 0x1A,
	Shield_1B	= 0x1B,
	
	# These are all unimplemented
	Max			= 0x1C,
	
	surf_1D		= 0x1D,
	surf_1E		= 0x1E,
	surf_1F		= 0x1F,
	surf_20		= 0x20,
	surf_21		= 0x21,
	surf_22		= 0x22,
	surf_23		= 0x23,
	surf_24		= 0x24,
	surf_25		= 0x25,
	surf_26		= 0x26,
	surf_27		= 0x27,
	surf_28		= 0x28,
	surf_29		= 0x29,
	surf_2A		= 0x2A,
	surf_2B		= 0x2B,
	surf_2C		= 0x2C,
	surf_2D		= 0x2D,
	surf_2E		= 0x2E,
	surf_2F		= 0x2F,
	
	None		= 0x30,
}

static var _surface_resistances: PackedFloat32Array
static func surface_resistance(surf: Surface) -> float:
	if _surface_resistances.is_empty():
		_surface_resistances = FileAccess.get_file_as_bytes("res://proprietary/loc/terrain_resistances.data").to_float32_array()
	
	if surf < 0 || surf >= _surface_resistances.size():
		return 0.9
	
	return _surface_resistances[surf]

static func is_road(surf: Surface) -> bool:
	return surf == Surface.Road_05 || surf == Surface.Concrete_07

static func is_water(surf: Surface) -> bool:
	return surf == Surface.Water_10

static func surface_from_ray(ray: Dictionary) -> Surface:
	if ray.is_empty():
		return Surface.None
	
	var collider := ray.collider as CollisionObject3D
	var owner_id := collider.shape_find_owner(ray.shape)
	var shape := collider.shape_owner_get_owner(owner_id)
	if !(shape is BBHitboxShape):
		return Surface.None
	
	var hbx_shape := shape as BBHitboxShape
	return hbx_shape.get_surface(ray)

@export var skeleton: Skeleton3D

func on_skeleton_updated() -> void:
	for n in get_children():
		if n is BBHitboxShape:
			var hbs := n as BBHitboxShape
			var bone_id : int = skeleton.find_bone(hbs.bone_name)
			if bone_id >= 0:
				hbs.transform = skeleton.get_bone_global_pose(bone_id)

func set_debug_color(color: Color) -> void:
	for n in get_children():
		if n is BBHitboxShape:
			var hbs := n as BBHitboxShape
			hbs.debug_color = Color(color, hbs.debug_color.a)

func set_disabled(disabled: bool) -> void:
	for n in get_children():
		if n is BBHitboxShape:
			var hbs := n as BBHitboxShape
			hbs.disabled = disabled
