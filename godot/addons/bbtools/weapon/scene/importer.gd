@tool
class_name EditorSceneFormatImporterBBWeapon extends EditorSceneFormatImporter

func _get_extensions() -> PackedStringArray:
	return ["weapon_scene"]

func _get_import_flags() -> int:
	return IMPORT_SCENE

func _import_scene(path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	
	var weapon := BBWeapon.new()
	weapon.name = "Weapon"
	
	var texture := load("res://proprietary/loc/textures/SCIDOBJ.dds") as Texture2D
	
	var material := ShaderMaterial.new()
	material.shader = load("res://addons/bbtools/weapon/scene/weapon.gdshader") as Shader
	material.set_shader_parameter("albedo_texture", texture)
	weapon.material = material
	
	var id := file.get_8()
	var _type := file.get_8()
	
	var config_path := base_path.path_join("%02d.weapon" % id)
	var config := load(config_path) as BBWeaponConfig
	weapon.config = config
	
	weapon.flags = file.get_32()
	
	var model_scene_path := file.get_pascal_string()
	var model_scene := load(model_scene_path) as PackedScene
	var model := model_scene.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as Node3D
	model.name = "Model"
	
	weapon.add_child(model)
	model.owner = weapon
	weapon.model = model
	
	weapon.set_editable_instance(model, true)
	
	var anim_player := model.get_node_or_null(^"AnimationPlayer") as AnimationPlayer
	if anim_player:
		var anim_tree := AnimationTree.new()
		anim_tree.name = "AnimationTree"
		anim_tree.deterministic = false
		
		model.add_child(anim_tree)
		anim_tree.owner = weapon
		
		anim_tree.advance_expression_base_node = anim_tree.get_path_to(weapon)
		anim_tree.anim_player = anim_tree.get_path_to(anim_player)
		
		if weapon.flags & BBWeapon.Flags.EquipAnim && weapon.flags & BBWeapon.Flags.AttackAnim:
			anim_tree.tree_root = load("res://addons/bbtools/weapon/anim_state_machine/equip_attack.tres") as AnimationNodeStateMachine
		elif weapon.flags & BBWeapon.Flags.AttackAnim:
			anim_tree.tree_root = load("res://addons/bbtools/weapon/anim_state_machine/attack.tres") as AnimationNodeStateMachine
		elif weapon.flags & BBWeapon.Flags.EquipAnim:
			anim_tree.tree_root = load("res://addons/bbtools/weapon/anim_state_machine/equip.tres") as AnimationNodeStateMachine
		else:
			push_warning("Weapon animation missing equip and attack flags")
	
	var mech_effect_path := file.get_pascal_string()
	if !mech_effect_path.is_empty():
		weapon.mech_effect = load(mech_effect_path) as BBEffectConfigGroup
	
	var charging_effect_path := file.get_pascal_string()
	if !charging_effect_path.is_empty():
		weapon.charging_effect = load(charging_effect_path) as BBEffectConfigGroup
	
	var firing_effect_count := file.get_32()
	for i in firing_effect_count:
		var firing_effect_path := file.get_pascal_string()
		var firing_effect := load(firing_effect_path) as BBEffectConfigGroup
		weapon.firing_effects.append(firing_effect)
	
	var smoke_effect_path := file.get_pascal_string()
	if !smoke_effect_path.is_empty():
		weapon.smoke_effect = load(smoke_effect_path) as BBEffectConfigGroup
	
	var casing_effect_path := file.get_pascal_string()
	if !casing_effect_path.is_empty():
		weapon.casing_effect = load(casing_effect_path) as BBEffectConfigGroup
	
	var light_offset := Vector3(file.get_float(), file.get_float(), file.get_float())
	
	var light_scenes: Array[PackedScene]
	
	weapon.flash_count = file.get_32()
	for i in weapon.flash_count:
		var light_path := file.get_pascal_string()
		var light_scene := load(light_path) as PackedScene
		light_scenes.append(light_scene)
	
	var muzzle_attachment := BoneAttachment3D.new()
	muzzle_attachment.name = "Muzzle"
	muzzle_attachment.bone_name = "special_0"
	
	var skeleton := model.get_node(^"Skeleton3D") as Skeleton3D
	skeleton.add_child(muzzle_attachment)
	muzzle_attachment.owner = weapon
	
	for y in config.muzzle_count.y:
		for x in config.muzzle_count.x:
			var muzzle := y * config.muzzle_count.x + x
			
			for i in weapon.flash_count:
				var light := light_scenes[i].instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE) as BBPointLight
				light.name = "Flash_%d%s" % [muzzle, char(65 + i)]
				light.autostart = false
				light.oneshot = false
				
				var muzzle_offset := config.get_muzzle_offset(muzzle)
				light.position = Vector3(muzzle_offset.x, muzzle_offset.y, 0.0)
				
				muzzle_attachment.add_child(light)
				light.owner = weapon
				
				weapon.flashes.append(light)
	
	return weapon
