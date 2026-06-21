extends Node3D

const mech_directory := "res://proprietary/loc/mechs/"
const mwep_directory := "res://proprietary/loc/weapons/main/"
const swep_directory := "res://proprietary/loc/weapons/sub/"

@onready var mech_option := $TopLeft/MechList as OptionButton

@onready var mwep_option := $TopLeft/MWepList as OptionButton
@onready var mwep_slot := $TopLeft/MWepSlot as SpinBox
@onready var mwep_switch := $TopLeft/MWepSwitch as Button
@onready var mwep_fire := $TopLeft/MWepFire as Button

@onready var swep_option := $TopLeft/SWepList as OptionButton
@onready var swep_slot := $TopLeft/SWepSlot as SpinBox
@onready var swep_switch := $TopLeft/SWepSwitch as Button
@onready var swep_fire := $TopLeft/SWepFire as Button

@onready var standing := $TopRight/Standing as CheckBox
@onready var fully_deployed := $TopRight/FullyDeployed as CheckBox
@onready var movement_text := $TopRight/MovementText as LineEdit
@onready var slide_text := $TopRight/SlideText as LineEdit
@onready var fall_text := $TopRight/FallText as LineEdit

var mech: BBMech

var projectile_flying: bool
@onready var projectile := $Projectile as Node3D

func _ready() -> void:
	var mech_dirs := DirAccess.get_directories_at(mech_directory)
	for mech_id_str in mech_dirs:
		var resource_list := ResourceLoader.list_directory(mech_directory + mech_id_str)
		for resource in resource_list:
			if !resource.ends_with(".mech_scene"):
				continue
			
			var mech_path := mech_directory + mech_id_str + "/" + resource
			
			mech_option.add_item("Mech_%s" % mech_id_str)
			mech_option.set_item_metadata(mech_option.item_count - 1, mech_path)
	
	_add_option_resources(mwep_directory, ".weapon", mwep_option)
	_add_option_resources(swep_directory, ".weapon", swep_option)

func _process(_delta: float) -> void:
	standing.button_pressed = mech && mech.is_standing()
	fully_deployed.button_pressed = mech && mech.is_deployed()

func _physics_process(delta: float) -> void:
	if projectile_flying:
		projectile.translate(Vector3.BACK * (delta * 180.0))

func _add_option_resources(directory: String, extension: String, option: OptionButton) -> void:
	option.add_item("NONE")
	option.set_item_metadata(option.item_count - 1, "INVALID_PATH")
	
	var res_list := ResourceLoader.list_directory(directory)
	res_list.sort()
	for res_name in res_list:
		if !res_name.ends_with(extension):
			continue
		
		option.add_item(res_name.get_slice(".", 0))
		option.set_item_metadata(option.item_count - 1, directory + res_name)

func _on_mech_switch_pressed() -> void:
	if mech:
		remove_child(mech)
		mech.queue_free()
	
	var mech_path := mech_option.get_selected_metadata() as String
	var valid_mech := ResourceLoader.exists(mech_path)
	
	mwep_option.visible = valid_mech
	mwep_slot.visible = valid_mech
	mwep_switch.visible = valid_mech
	mwep_fire.visible = valid_mech
	
	swep_option.visible = valid_mech
	swep_slot.visible = valid_mech
	swep_switch.visible = valid_mech
	swep_fire.visible = valid_mech
	
	if !valid_mech:
		return
	
	var mech_scene := load(mech_path) as PackedScene
	mech = mech_scene.instantiate() as BBMech
	
	var chassis := mech.chassis
	
	var anim_player := chassis.get_node_or_null(^"AnimationPlayer") as AnimationPlayer
	if anim_player:
		anim_player.play("Anim_0")
	
	add_child(mech)

func _on_mwep_switch_pressed() -> void:
	if !mech:
		return
	
	var mwep_path := mwep_option.get_selected_metadata() as String
	var mwep_cfg := load(mwep_path) as BBWeaponConfig if ResourceLoader.exists(mwep_path) else null
	var mwep_slot_idx := int(mwep_slot.value)
	
	mech.set_main_weapon(mwep_cfg, mwep_slot_idx)

func _on_mwep_fire_pressed() -> void:
	if !mech:
		return
	
	_fire_weapon(mech.mweps[mech.mwep_idx])

func _on_swep_switch_pressed() -> void:
	if !mech:
		return
	
	var swep_path := swep_option.get_selected_metadata() as String
	var swep_cfg := load(swep_path) as BBWeaponConfig if ResourceLoader.exists(swep_path) else null
	var swep_slot_idx := int(swep_slot.value)
	
	mech.set_sub_weapon(swep_cfg, swep_slot_idx)

func _on_swep_fire_pressed() -> void:
	if !mech:
		return
	
	_fire_weapon(mech.sweps[mech.swep_idx])

func _fire_weapon(wep: BBWeapon) -> void:
	if !wep:
		return
	
	for n in projectile.get_children():
		projectile.remove_child(n)
		n.queue_free()
	
	wep.fire()
	
	projectile.transform = wep.get_muzzle_transform(0)
	
	var p := wep.config.projectile_scene.instantiate()
	projectile.add_child(p)
	
	projectile_flying = true

func _on_projectile_freeze_pressed() -> void:
	projectile_flying = false

func _on_movement_switch_pressed() -> void:
	if !mech:
		return
	
	var parts := movement_text.text.split(":")
	if parts.is_empty():
		return
	
	var speed_ms := -1.0
	if parts.size() > 1:
		speed_ms = float(parts[1])
	
	mech.set_movement(parts[0], speed_ms)

func _on_slide_switch_pressed() -> void:
	if !mech:
		return
	
	mech.set_slide(slide_text.text)

func _on_fall_switch_pressed() -> void:
	if !mech:
		return
	
	mech.set_fall(fall_text.text)

func _on_rising_toggled(toggled_on: bool) -> void:
	if !mech:
		return
	
	mech.rising = toggled_on

func _on_deployed_toggled(toggled_on: bool) -> void:
	if !mech:
		return
	
	mech.deployed = toggled_on

func _on_hatch_toggled(toggled_on: bool) -> void:
	if !mech:
		return
	
	mech.hatch_closed = toggled_on

func _on_drop_left_tank_pressed() -> void:
	mech.drop_sub_tank(false)

func _on_drop_right_tank_pressed() -> void:
	mech.drop_sub_tank(true)

func _on_drop_armor_pressed() -> void:
	mech.drop_opt_armor()
