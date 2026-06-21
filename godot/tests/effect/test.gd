extends Node3D

const effect_directory := "res://proprietary/loc/effects/effects/"
const mech_directory := "res://proprietary/loc/mechs/"

@onready var effect_option := $VBoxContainer/EffectList as OptionButton
@onready var mech_option := $VBoxContainer/MechList as OptionButton
@onready var anim_option := $VBoxContainer/AnimList as OptionButton
@onready var anim_play_button := $VBoxContainer/AnimPlay as Button
@onready var sequence_flags_label := $SequenceFlags as Label
@onready var mech_camo_option := $VBoxContainer/MechCamo as OptionButton

var mech: BBMech
var anim_player: AnimationPlayer

# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	var effect_list := ResourceLoader.list_directory(effect_directory)
	effect_list.sort()
	for i in effect_list.size():
		var effect_file := effect_list[i]
		var effect_name := effect_file.get_slice(".", 0)
		
		effect_option.add_item("%s | %03d" % [effect_name, i])
		effect_option.set_item_metadata(effect_option.item_count - 1, effect_directory + effect_file)
	
	var mech_list := ResourceLoader.list_directory(mech_directory)
	mech_list.sort()
	for mech_name in mech_list:
		if !mech_name.ends_with(".mech_scene"):
			continue
		
		mech_option.add_item(mech_name.get_slice(".", 0))
		mech_option.set_item_metadata(mech_option.item_count - 1, mech_directory + mech_name)

func _on_effect_play_pressed() -> void:
	var eff_path := effect_option.get_selected_metadata() as String
	var eff_cfg := load(eff_path) as BBEffectConfigGroup
	BBEffectManager.spawn(eff_cfg, {}, self)

func _on_effect_reset_pressed() -> void:
	BBEffectManager.reset()

func _on_mech_switch_pressed() -> void:
	if mech:
		remove_child(mech)
		mech.queue_free()
	
	var mech_path := mech_option.get_selected_metadata() as String
	var mech_scene := load(mech_path) as PackedScene
	mech = mech_scene.instantiate() as BBMech
	
	var chassis := mech.chassis
	
	mech_camo_option.select(0)
	mech_camo_option.show()
	
	anim_option.clear()
	anim_player = chassis.get_node_or_null(^"AnimationPlayer") as AnimationPlayer
	if anim_player:
		anim_option.show()
		anim_play_button.show()
		
		for anim_name in anim_player.get_animation_list():
			anim_option.add_item(anim_name)
			anim_option.set_item_metadata(anim_option.item_count - 1, anim_name)
		
		anim_player.play("Anim_0")
	else:
		anim_option.hide()
		anim_play_button.hide()
	
	if chassis.has_node(^"AnimationSequence"):
		var anim_sequence := chassis.get_node(^"AnimationSequence") as BBAnimationSequence
		anim_sequence.flags_event.connect(_on_animation_sequence_flags_event)
	
	add_child(mech)

func _on_anim_play_pressed() -> void:
	_on_animation_sequence_flags_event(0)
	var anim_name := anim_option.get_selected_metadata() as String
	anim_player.play(anim_name)

func _on_animation_sequence_flags_event(flags: int) -> void:
	sequence_flags_label.text = "Sequence Flags: %04X" % flags

func _on_mech_camo_item_selected(index: int) -> void:
	mech.current_mesh = clampi(index, 0, 3)
