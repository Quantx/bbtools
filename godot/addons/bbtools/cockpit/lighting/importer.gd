@tool
class_name EditorSceneFormatImporterBBCockpitLighting extends EditorSceneFormatImporter

func _get_extensions() -> PackedStringArray:
	return ["cockpit_lighting"]

func _get_import_flags() -> int:
	return IMPORT_SCENE

func _import_scene(path: String, _flags: int, _options: Dictionary) -> Node:
	var base_path := path.get_base_dir()
	
	var file := FileAccess.open(path, FileAccess.READ)
	
	var root := BBCockpitLighting.new()
	root.name = "Lighting"
	
	var sun := DirectionalLight3D.new()
	sun.name = "Sun"
	sun.light_cull_mask = 2
	sun.layers = 2
	
	root.add_child(sun)
	sun.owner = root
	
	# Each animation uses some number of point lights less than or equal to this value
	# Unused point lights are hidden
	var point_light_max := file.get_32()
	for i in point_light_max:
		var light := OmniLight3D.new()
		light.name = "%d" % i
		light.omni_attenuation = 2.0
		light.light_cull_mask = 2
		light.layers = 2
		
		root.add_child(light)
		light.owner = root
	
	var anim_player := AnimationPlayer.new()
	anim_player.name = "AnimationPlayer"
	
	root.add_child(anim_player)
	anim_player.owner = root
	
	var library := AnimationLibrary.new()
	
	var cockpit_closed_time := file.get_float()
	
	var animation_count := file.get_32()
	for animation_idx in animation_count:
		var animation := Animation.new()
		animation.length = 0.0
		animation.step = 0.05 # 20 FPS
		
		# Add a track which indicates whether or not the hatch is open
		# Animation 5 is Ejecting, the hatch could either be open or closed here so do nothing
		if animation_idx <= 4:
			var track_cockpit_closed_idx := animation.add_track(Animation.TYPE_METHOD)
			animation.track_set_path(track_cockpit_closed_idx, ".")
			animation.track_set_interpolation_type(track_cockpit_closed_idx, Animation.INTERPOLATION_NEAREST)
			
			match animation_idx:
				0: # Closing
					animation.track_insert_key(track_cockpit_closed_idx, 0.0, {"method": "_hatch_callback", "args": [true]}, 0.0)
					animation.track_insert_key(track_cockpit_closed_idx, cockpit_closed_time, {"method": "_hatch_callback", "args": [false]}, 0.0)
				3: # Open
					animation.track_insert_key(track_cockpit_closed_idx, 0.0, {"method": "_hatch_callback", "args": [true]}, 0.0)
				1, 2, 4: # Boot, Systems, Active
					animation.track_insert_key(track_cockpit_closed_idx, 0.0, {"method": "_hatch_callback", "args": [false]}, 0.0)
		
		var scene_frame_count := file.get_32()
		if animation_idx == 5:
			animation.loop_mode = Animation.LOOP_LINEAR
			file.get_buffer(scene_frame_count * 44)
		else:
			var track_ambient_color_idx := animation.add_track(Animation.TYPE_VALUE)
			animation.track_set_path(track_ambient_color_idx, ".:environment:ambient_light_color")
			
			var track_sun_color_idx := animation.add_track(Animation.TYPE_VALUE)
			animation.track_set_path(track_sun_color_idx, "Sun:light_color")
			
			var track_sun_rotation := animation.add_track(Animation.TYPE_ROTATION_3D)
			animation.track_set_path(track_sun_rotation, "Sun:rotation")
			
			for frame_idx in scene_frame_count:
				var time := file.get_float()
				animation.length = maxf(animation.length, time)
				
				var ambient_color := Color(file.get_float(), file.get_float(), file.get_float())
				animation.track_insert_key(track_ambient_color_idx, time, ambient_color)
				
				var _ambient_energy := file.get_float() # Unused
				
				var sun_color := Color(file.get_float(), file.get_float(), file.get_float())
				animation.track_insert_key(track_sun_color_idx, time, sun_color)
				
				var _sun_energy := file.get_float() # Unused
				
				var sun_yaw := file.get_float()
				var sun_pitch := file.get_float()
				var sun_rotation := Quaternion.from_euler(Vector3(-sun_pitch, sun_yaw, 0.0))
				animation.track_insert_key(track_sun_rotation, time, sun_rotation)
		
		var point_light_count := file.get_32()
		for point_light_idx in point_light_max:
			var point_light_visible := point_light_idx < point_light_count
			
			var track_visible_idx := animation.add_track(Animation.TYPE_VALUE)
			animation.track_set_path(track_visible_idx, "%d:visible" % point_light_idx)
			animation.track_insert_key(track_visible_idx, 0.0, point_light_visible)
			
			if !point_light_visible:
				continue
			
			var track_pos_idx := animation.add_track(Animation.TYPE_POSITION_3D)
			animation.track_set_path(track_pos_idx, "%d:position" % point_light_idx)
			
			var track_color_idx := animation.add_track(Animation.TYPE_VALUE)
			animation.track_set_path(track_color_idx, "%d:light_color" % point_light_idx)
			
			var track_energy_idx := animation.add_track(Animation.TYPE_VALUE)
			animation.track_set_path(track_energy_idx, "%d:omni_range" % point_light_idx)
			
			var point_light_frame_count := file.get_32()
			for frame_idx in point_light_frame_count:
				var time := file.get_float()
				animation.length = maxf(animation.length, time)
				
				var position := Vector3(file.get_float(), file.get_float(), file.get_float())
				animation.track_insert_key(track_pos_idx, time, position)
				
				var color := Color(file.get_float(), file.get_float(), file.get_float())
				animation.track_insert_key(track_color_idx, time, color)
				
				var energy := file.get_float()
				animation.track_insert_key(track_energy_idx, time, energy)
		
		library.add_animation(&"Anim_%d" % animation_idx, animation)
	
	anim_player.add_animation_library(&"", library)
	
	return root
