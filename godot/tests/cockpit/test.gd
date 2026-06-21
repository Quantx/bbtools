extends Node3D

const mission_base_path := "res://proprietary/loc/missions/"
const cockpit_base_path := "res://proprietary/loc/cockpits/"

@onready var mission_option := $VBoxContainer/MissionList as OptionButton
@onready var cockpit_option := $VBoxContainer/CockpitList as OptionButton
@onready var poses_spinbox := $VBoxContainer/Poses as SpinBox
@onready var eject_button := $VBoxContainer/Eject as Button
@onready var mech_viewport := $MechViewport as SubViewport
@onready var mech_camera := $MechViewport/Camera as Camera3D

var mission: BBMission
var cockpit: BBCockpit

func _ready() -> void:
	var window := get_window()
	window.size_changed.connect(_window_size_changed, CONNECT_APPEND_SOURCE_OBJECT)
	mech_viewport.size = window.size
	
	var mission_dirs := DirAccess.get_directories_at(mission_base_path)
	for mission_id_str in mission_dirs:
		var resource_list := ResourceLoader.list_directory(mission_base_path + mission_id_str)
		for resource in resource_list:
			if !resource.ends_with(".mission_scene"):
				continue
			
			var mission_path := mission_base_path + mission_id_str + "/" + resource
			
			mission_option.add_item("Mission %s" % mission_id_str)
			mission_option.set_item_metadata(cockpit_option.item_count - 1, mission_path)
	
	var cockpit_list := ResourceLoader.list_directory(cockpit_base_path)
	for cockpit_name in cockpit_list:
		cockpit_option.add_item(cockpit_name.get_slice(".", 0))
		cockpit_option.set_item_metadata(cockpit_option.item_count - 1, cockpit_base_path + cockpit_name)

func _window_size_changed(window: Window) -> void:
	mech_viewport.size = window.size

func _on_mission_switch_pressed() -> void:
	if mission:
		remove_child(mission)
		mission.queue_free()
	
	var mission_path := mission_option.get_selected_metadata() as String
	var mission_scene := load(mission_path) as PackedScene
	mission = mission_scene.instantiate() as BBMission
	
	add_child(mission)
	
	var br := mission.get_node(^"Objects/BR_0") as Node3D
	mech_camera.transform = br.transform.translated_local(Vector3.UP * 18.0)

func _on_cockpit_switch_pressed() -> void:
	if cockpit:
		poses_spinbox.hide()
		eject_button.hide()
		
		remove_child(cockpit)
		cockpit.queue_free()
	
	var cockpit_path := cockpit_option.get_selected_metadata() as String
	var cockpit_scene := load(cockpit_path) as PackedScene
	cockpit = cockpit_scene.instantiate() as BBCockpit
	
	poses_spinbox.value = 0
	poses_spinbox.max_value = cockpit.pilot_poses.size() - 1
	poses_spinbox.visible = !cockpit.pilot_poses.is_empty()
	
	eject_button.show()
	eject_button.button_pressed = false
	
	add_child(cockpit)
	
	cockpit.set_background_texture(mech_viewport.get_texture())
	cockpit.pilot_camera.make_current()

func _on_poses_value_changed(value: float) -> void:
	cockpit.select_pose(int(value))

func _on_eject_toggled(ejecting: bool) -> void:
	if ejecting:
		cockpit.eject()
	else:
		cockpit.reset()
		poses_spinbox.value = 0
