@tool
extends EditorPlugin

var mission_importer: EditorSceneFormatImporterBBMission
var mech_importer: EditorSceneFormatImporterBBMech
var weapon_importer: EditorSceneFormatImporterBBWeapon
var projectile_importer: EditorSceneFormatImporterBBProjectile
var point_light_importer: EditorSceneFormatImporterBBPointLight
var trail_importer: EditorSceneFormatImporterBBRibbonTrail
var cockpit_lighting_importer: EditorSceneFormatImporterBBCockpitLighting
var cockpit_importer: EditorSceneFormatImporterBBCockpit
var gltf_extension: GLTFDocumentExtensionBBTools

func _enable_plugin() -> void:
	# Add autoloads here
	pass

func _disable_plugin() -> void:
	# Remove autoloads here
	pass

func _enter_tree() -> void:
	# Initialization of the plugin goes here.
	if Engine.is_editor_hint():
		mission_importer = EditorSceneFormatImporterBBMission.new()
		add_scene_format_importer_plugin(mission_importer)
		
		mech_importer = EditorSceneFormatImporterBBMech.new()
		add_scene_format_importer_plugin(mech_importer)
		
		weapon_importer = EditorSceneFormatImporterBBWeapon.new()
		add_scene_format_importer_plugin(weapon_importer)
		
		projectile_importer = EditorSceneFormatImporterBBProjectile.new()
		add_scene_format_importer_plugin(projectile_importer)
		
		point_light_importer = EditorSceneFormatImporterBBPointLight.new()
		add_scene_format_importer_plugin(point_light_importer)
		
		trail_importer = EditorSceneFormatImporterBBRibbonTrail.new()
		add_scene_format_importer_plugin(trail_importer)
		
		cockpit_lighting_importer = EditorSceneFormatImporterBBCockpitLighting.new()
		add_scene_format_importer_plugin(cockpit_lighting_importer)
		
		cockpit_importer = EditorSceneFormatImporterBBCockpit.new()
		add_scene_format_importer_plugin(cockpit_importer)
		
		gltf_extension = GLTFDocumentExtensionBBTools.new()
		GLTFDocument.register_gltf_document_extension(gltf_extension)

func _exit_tree() -> void:
	if mission_importer:
		remove_scene_format_importer_plugin(mission_importer)
	
	if mech_importer:
		remove_scene_format_importer_plugin(mech_importer)
	
	if weapon_importer:
		remove_scene_format_importer_plugin(weapon_importer)
	
	if projectile_importer:
		remove_scene_format_importer_plugin(projectile_importer)
	
	if point_light_importer:
		remove_scene_format_importer_plugin(point_light_importer)
	
	if trail_importer:
		remove_scene_format_importer_plugin(trail_importer)
	
	if cockpit_lighting_importer:
		remove_scene_format_importer_plugin(cockpit_lighting_importer)
	
	if cockpit_importer:
		remove_scene_format_importer_plugin(cockpit_importer)
	
	if gltf_extension:
		GLTFDocument.unregister_gltf_document_extension(gltf_extension)
