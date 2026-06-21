class_name BBCockpit extends Node3D

@export var chassis_textures: Array[Texture2D]
@export var display_textures: Array[Texture3D]

@export var chassis_material: StandardMaterial3D
@export var display_material: ShaderMaterial

@export var background_material: ShaderMaterial

# Startup
var startup_state: int

# Ejector
var ejecting: bool

# Weapons
var mwep_index: int
var swep_index: int

@export var lighting: BBCockpitLighting
@export var background: MeshInstance3D

@export var animation_trees: Array[AnimationTree]

@export_group("Monitors")
@export var monitor_main: Node3D
@export var monitor_sub: Node3D
@export var monitor_multi: Node3D
var monitor_multi_closed: bool

@export_group("Pilot")
@export var pilot_root: Node3D
@export var pilot_eject: Node3D
@export var pilot_camera: Camera3D
@export var pilot_poses: Array[Marker3D]
var _pose_index: int
var _pose_camera_switch_distance_reciprocal: float
var _pose_camera_switch_rotation: Quaternion

@export_group("Tuner")
@export var tuner_root: Node3D
@export var tuner: BBSkeletonModifierTuner
var tuner_index_current: int
var tuner_index_target: int

var comms_index: int = -1

@export_group("Dials")
@export var dials_root: Node3D
@export var dials: BBSkeletonModifierDials

@export_group("Indicators")
enum Indicators {
	Eject,
	CollisionCenter,
	CollisionLeft,
	CollisionRight,
	Tipping,
	Tank2Red,
	Tank2Green,
	Tank1Red,
	Tank1Green,
	Override,
}

@export var indicator_effects: Array[BBEffectConfigGroup]
@export var indicator_positions: PackedVector3Array
var _indicators: Array[BBEffect]

const comm_light_count := 5
@export var comm_light_effects: Array[BBEffectConfigGroup]
@export var comm_light_positions: PackedVector3Array
var _comm_lights: Array[BBEffect]

func reset() -> void:
	startup_state = 0
	
	ejecting = false
	
	mwep_index = 0
	swep_index = 0
	
	monitor_multi_closed = false
	
	_pose_index = 0
	
	tuner_index_current = 0
	tuner_index_target = 0
	
	comms_index = -1
	
	select_pose(0)
	pilot_camera.transform = pilot_poses[0].transform
	
	# Reset all animation trees
	for anim_tree in animation_trees:
		anim_tree.active = false
		anim_tree.active = true
	
	# Turn off all indicators
	for indicator in _indicators:
		if indicator:
			indicator.enabled = false
	
	for comm_light in _comm_lights:
		if comm_light:
			comm_light.enabled = false

func _ready() -> void:
	# Connect Animation Trees to Animation Sequences
	for anim_tree in animation_trees:
		var anim_sequence := anim_tree.get_node_or_null(^"../AnimationSequence") as BBAnimationSequence
		if anim_sequence:
			anim_tree.mixer_applied.connect(anim_sequence.on_mixer_applied)
	
	for n in $Monitor.get_children():
		var mesh_inst := n.get_node_or_null(^"Skeleton3D/0") as MeshInstance3D
		if mesh_inst:
			mesh_inst.layers = 2
	
	for n in $Chassis.get_children():
		var mesh_inst := n.get_node_or_null(^"Skeleton3D/0") as MeshInstance3D
		if mesh_inst:
			mesh_inst.layers = 2
			mesh_inst.material_override = chassis_material
	
	for n in $Display.get_children():
		var mesh_inst := n.get_node_or_null(^"Skeleton3D/0") as MeshInstance3D
		if mesh_inst:
			mesh_inst.layers = 2
			mesh_inst.material_override = display_material
	
	if !Engine.is_editor_hint():
		if tuner_root:
			if tuner:
				tuner.reparent(tuner_root.get_node(^"Skeleton3D"))
				
				var mesh_inst := tuner_root.get_node(^"Skeleton3D/0") as MeshInstance3D
				mesh_inst.sorting_offset = 0.1
			else:
				var anim_tree := tuner_root.get_node(^"AnimationTree") as AnimationTree
				var playback := anim_tree.get("parameters/playback") as AnimationNodeStateMachinePlayback
				playback.state_started.connect(_tuner_anim_state_started)
		
		if dials && dials_root:
			dials.reparent(dials_root.get_node(^"Skeleton3D"))
			
			var mesh_inst := dials_root.get_node(^"Skeleton3D/0") as MeshInstance3D
			mesh_inst.sorting_offset = 0.1
		
		var pilot_eject_skeleton := pilot_eject.get_node(^"Skeleton3D") as Skeleton3D
		pilot_eject_skeleton.skeleton_updated.connect(_on_pilot_eject_skeleton_updated, CONNECT_APPEND_SOURCE_OBJECT)
		
		lighting.hatch_changed.connect(_on_hatch_changed)
		
		_indicators.resize(Indicators.size())
		for i in Indicators.size():
			if !indicator_effects[i]:
				continue
			
			var args := {
				"position": indicator_positions[i]
			}
			
			var effects := BBEffectManager.spawn(indicator_effects[i], args, self) as Array[BBEffect]
			assert(effects.size() == 1)
			_indicators[i] = effects[0]
		
		if comm_light_effects:
			_comm_lights.resize(BBCockpit.comm_light_count)
			for i in BBCockpit.comm_light_count:
				var args := {
					"position": comm_light_positions[i]
				}
				
				var effects := BBEffectManager.spawn(comm_light_effects[i], args, self) as Array[BBEffect]
				assert(effects.size() == 1)
				_comm_lights[i] = effects[0]
	
	reset()

func _process(delta: float) -> void:
	if !pilot_camera.position.is_equal_approx(pilot_poses[_pose_index].position):
		pilot_camera.position = pilot_camera.position.move_toward(pilot_poses[_pose_index].position, delta)
		var weight := _camera_pose_distance() * _pose_camera_switch_distance_reciprocal
		pilot_camera.quaternion = pilot_poses[_pose_index].quaternion.slerp(_pose_camera_switch_rotation, weight)

func _camera_pose_distance() -> float:
	return pilot_camera.position.distance_to(pilot_poses[_pose_index].position)

func select_pose(index: int) -> void:
	if index < 0 || index >= pilot_poses.size():
		push_error("Invalid pilot pose %d, must be between [0, %d]" % [index, pilot_poses.size()])
		return
	
	if ejecting:
		return
	
	_pose_index = index
	
	# This works even if the distance is zero
	var distance := _camera_pose_distance()
	_pose_camera_switch_distance_reciprocal = 0.0 if is_zero_approx(distance) else 1.0 / distance
	_pose_camera_switch_rotation = pilot_camera.quaternion

func _on_pilot_eject_skeleton_updated(skeleton: Skeleton3D) -> void:
	var bone_idx := skeleton.find_bone("1")
	pilot_root.transform = skeleton.get_bone_global_pose(bone_idx)

func _tuner_anim_state_started(state: StringName) -> void:
	if !state.begins_with("Down") && !state.begins_with("Up"):
		return
	
	var index_string := state.get_slice(" ", 1)
	assert(index_string.is_valid_int())
	tuner_index_current = int(index_string)

func eject() -> void:
	if ejecting:
		return
	
	select_pose(0)
	ejecting = true

func set_background_texture(texture: Texture2D) -> void:
	background_material.set_shader_parameter("background_albedo", texture)

func _on_hatch_changed() -> void:
	background.visible = lighting.hatch_open

func set_indicator(indicator: Indicators, enabled: bool) -> void:
	if startup_state == 4 && _indicators[indicator]:
		_indicators[indicator].enabled = enabled

func set_comm_light(index: int, enabled: bool) -> void:
	if index < 0 || index >= comm_light_count:
		push_error("Invalid comms index")
		return
	
	if _comm_lights.size() != comm_light_count:
		return
	
	if startup_state == 4 && _comm_lights[index]:
		_comm_lights[index].enabled = enabled
