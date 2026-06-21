@tool
class_name GLTFDocumentExtensionBBTools extends GLTFDocumentExtension

const ext_name := "bbtools"

func _get_supported_extensions() -> PackedStringArray:
	return [ext_name]

func _parse_node_extensions(_state: GLTFState, gltf_node: GLTFNode, extensions: Dictionary) -> Error:
	gltf_node.set_additional_data(ext_name, extensions.get(ext_name))
	return OK

func _import_post_parse(state: GLTFState) -> Error:
	if "animations" in state.json:
		var animations := state.get_animations()
		var animations_json := state.json.animations as Array
		for i in animations_json.size():
			var animation_json := animations_json[i] as Dictionary
			if "extensions" in animation_json:
				animations[i].set_additional_data(ext_name, animation_json.extensions.get(ext_name))
	
	return OK

func _get_mirror_node(state: GLTFState, gltf_node: GLTFNode) -> GLTFNode:
	var bbtools_var := gltf_node.get_additional_data(ext_name)
	if !(bbtools_var is Dictionary):
		return null
	
	var bbtools := bbtools_var as Dictionary
	if "mirror" not in bbtools:
		return null
		return null
	
	return state.get_nodes()[bbtools.mirror as int]

func _import_post(state: GLTFState, root: Node) -> Error:
	_apply_skeleton_metadata(state, root)
	
	var sequence := _generate_sequence(state, root)
	
	var hbx_shot := _generate_hitbox(state, root, BBHitbox.Layer.ShotHitbox)
	if hbx_shot:
		hbx_shot.name = "ShotHitbox"
		hbx_shot.collision_layer = 1
		hbx_shot.collision_mask = 0
		hbx_shot.set_debug_color(Color.DARK_RED)
		if sequence:
			sequence.raycast_excludes.append(hbx_shot)
	
	var hbx_phys := _generate_hitbox(state, root, BBHitbox.Layer.PhysHitbox)
	if hbx_phys:
		hbx_phys.name = "PhysHitbox"
		hbx_phys.collision_layer = 2
		hbx_phys.collision_mask = 0
		hbx_phys.set_debug_color(Color.DARK_BLUE)
		if sequence:
			sequence.raycast_excludes.append(hbx_phys)
	
	return OK

func _apply_skeleton_metadata(state: GLTFState, root: Node) -> void:
	var skeleton := root.get_node_or_null(^"Skeleton3D") as Skeleton3D
	if !skeleton:
		return
	
	for gltf_node in state.get_nodes():
		var bone_idx := skeleton.find_bone(gltf_node.original_name)
		if bone_idx < 0:
			continue
		
		var bbtools_var := gltf_node.get_additional_data(ext_name)
		if !(bbtools_var is Dictionary):
			continue
		
		var bbtools_ext := bbtools_var as Dictionary
		
		var meta_var := bbtools_ext.get("meta")
		if !(meta_var is Dictionary):
			continue
		
		var meta := meta_var as Dictionary
		
		for k in meta.keys():
			skeleton.set_bone_meta(bone_idx, k, meta[k])
			
			#print("BONE %s, KEY '%s', META " % [skeleton.get_bone_name(bone_idx), k], skeleton.get_bone_meta(bone_idx, k))
			#print(skeleton.get_bone_meta_list(bone_idx))

func _generate_hitbox(state: GLTFState, root: Node, mask : BBHitbox.Layer) -> BBHitbox:
	var hbx_shapes: Array[BBHitboxShape]
	for gltf_node in state.get_nodes():
		var hbx_shape := _generate_hitbox_shape(state, gltf_node, mask)
		if hbx_shape:
			hbx_shapes.append(hbx_shape)
	
	if hbx_shapes.is_empty():
		return null
	
	var skel := root.get_node(^"Skeleton3D") as Skeleton3D
	
	var hbx := BBHitbox.new()
	hbx.skeleton = skel
	
	root.add_child(hbx)
	hbx.owner = root
	
	skel.skeleton_updated.connect(hbx.on_skeleton_updated, CONNECT_PERSIST)
	
	for hbx_shape in hbx_shapes:
		var bone_idx := skel.find_bone(hbx_shape.bone_name)
		assert(bone_idx >= 0)
		hbx_shape.transform = skel.get_bone_global_rest(bone_idx)
		
		hbx.add_child(hbx_shape)
		hbx_shape.owner = root
	
	return hbx

func _generate_hitbox_shape(state: GLTFState, gltf_node: GLTFNode, mask: BBHitbox.Layer) -> BBHitboxShape:
	var bbtools_var := gltf_node.get_additional_data(ext_name)
	if !(bbtools_var is Dictionary):
		return null
	
	var bbtools_ext := bbtools_var as Dictionary
	
	var hitbox_ext := bbtools_ext.get("hitbox")
	if !hitbox_ext:
		return null
	
	var buffer_views := state.get_buffer_views()
	var triangles := buffer_views[hitbox_ext.triangles].load_buffer_view_data(state).to_vector3_array()
	var layers := buffer_views[hitbox_ext.layers].load_buffer_view_data(state)
	var surfaces := buffer_views[hitbox_ext.surfaces].load_buffer_view_data(state)
	
	var quad_count := surfaces.size()
	
	var quads_data := BBHitboxQuadsData.new()
	quads_data.layers = layers
	quads_data.surfaces = surfaces
	
	var mask_triangles : PackedVector3Array
	for q in quad_count:
		var layer := layers.decode_u16(q * 2)
		if (layer & mask) == mask:
			# Append Quad
			mask_triangles.append_array(triangles.slice(q * 6, q * 6 + 6))
			
			quads_data.surfaces.append(surfaces[q])
			quads_data.layers.append(layers[q * 2])
			quads_data.layers.append(layers[q * 2 + 1])
	
	var concave_shape := ConcavePolygonShape3D.new()
	concave_shape.set_faces(mask_triangles)
	
	var hbx_shape := BBHitboxShape.new()
	hbx_shape.name = gltf_node.original_name
	hbx_shape.bone_name = gltf_node.original_name
	hbx_shape.shape = concave_shape
	hbx_shape.quads_data = quads_data
	
	return hbx_shape

const sequence_node_name := "AnimationSequence"
func _generate_sequence(state: GLTFState, root: Node) -> BBAnimationSequence:
	var animation_player := root.get_node_or_null(^"AnimationPlayer") as AnimationPlayer
	if !animation_player:
		return null
	
	var buffer_views := state.get_buffer_views()
	
	# TODO: Encode this dynamically
	var effect_rel_path := "../effects/effects/EFG%04d.efg"
	
	var effect_path := state.base_path.path_join(effect_rel_path)
	
	var any_seqeuence := false
	for gltf_animation in state.get_animations():
		var bbtools_var := gltf_animation.get_additional_data(ext_name)
		if !(bbtools_var is Dictionary):
			continue
		
		var bbtools := bbtools_var as Dictionary
		
		var animation := animation_player.get_animation(gltf_animation.original_name)
		animation.loop_mode = bbtools.loop_mode as Animation.LoopMode
		var mirrored := bbtools.mirrored as bool
		
		if "sequence" in bbtools:
			var sequence_ext := bbtools.sequence as Dictionary
			if "effects" in sequence_ext:
				_add_effects_track(state, mirrored, buffer_views[sequence_ext.effects].load_buffer_view_data(state), animation, effect_path)
				any_seqeuence = true
			
			if "sounds" in sequence_ext:
				_add_sounds_track(state, mirrored, buffer_views[sequence_ext.sounds].load_buffer_view_data(state), animation)
				any_seqeuence = true
			
			if "flags" in sequence_ext:
				_add_flags_track(state, buffer_views[sequence_ext.flags].load_buffer_view_data(state), animation)
				any_seqeuence = true
	
	if !any_seqeuence:
		return null
	
	var sequence := BBAnimationSequence.new()
	sequence.name = sequence_node_name
	sequence.skeleton = root.get_node(^"Skeleton3D") as Skeleton3D
	
	if "extensions" in state.json:
		var root_extensions := state.json.extensions as Dictionary
		if "bbtools" in root_extensions:
			var bbtools_ext = root_extensions.bbtools as Dictionary
			if "surface_effects" in bbtools_ext:
				sequence.surface_effects = buffer_views[bbtools_ext.surface_effects].load_buffer_view_data(state).to_int32_array()
				if sequence.surface_effects.size() != BBAnimationSequence.expected_surface_effects_size:
					push_error("surface_effects was size %d not %d" % [sequence.surface_effects.size(), BBAnimationSequence.expected_surface_effects_size])
	
	root.add_child(sequence)
	sequence.owner = root
	
	animation_player.mixer_applied.connect(sequence.on_mixer_applied, CONNECT_PERSIST)
	
	return sequence

func _add_effects_track(state: GLTFState, mirrored: bool, data: PackedByteArray, animation: Animation, effects_path: String) -> void:
	var frames_buf := StreamPeerBuffer.new()
	frames_buf.data_array = data
	
	var track_idx := animation.add_track(Animation.TYPE_METHOD)
	animation.track_set_path(track_idx, sequence_node_name)
	animation.track_set_interpolation_type(track_idx, Animation.INTERPOLATION_NEAREST)
	
	var frame_count := frames_buf.get_u32()
	for frame_idx in frame_count:
		var time := frames_buf.get_float()
		
		var effect_args_list: Array[Dictionary]
		
		var effect_count := frames_buf.get_u32()
		for effect_idx in effect_count:
			var idx := frames_buf.get_u16()
			var node_idx := frames_buf.get_u8()
			var flags := frames_buf.get_u32()
			var detach_delay := frames_buf.get_float()
			
			var gltf_node := state.get_nodes()[node_idx]
			if mirrored:
				var mirror_node := _get_mirror_node(state, gltf_node)
				if mirror_node:
					gltf_node = mirror_node
			
			var position := Vector3(frames_buf.get_float(), frames_buf.get_float(), frames_buf.get_float())
			if mirrored:
				position.x = -position.x
			
			var rotation := Vector3(frames_buf.get_float(), frames_buf.get_float(), frames_buf.get_float())
			
			var trans_rotation := Vector3(frames_buf.get_float(), frames_buf.get_float(), frames_buf.get_float())
			if mirrored:
				trans_rotation.y = -trans_rotation.y
				trans_rotation.z = -trans_rotation.z
			
			var effect_args := {
				"bone": gltf_node.original_name,
				"detach_delay": detach_delay,
				"position": position,
			}
			
			if flags & 0x10 == 0x10:
				# Preform a surface collision check
				effect_args.surface_idx = idx
			else:
				# Surface idx is actually an effect ID (NOT AN EFFECT IDX)
				var path := effects_path % idx
				if ResourceLoader.exists(path):
					effect_args.effect = load(path) as BBEffectConfigGroup
				else:
					push_warning("Sequence for Animation %s is missing effect file: %s" % [animation.resource_name, path])
			
			if flags & 0x2 == 0x2:
				effect_args.rotation = rotation
			
			if flags & 0x4 == 0x4:
				effect_args.trans_rotation = trans_rotation
			
			effect_args_list.append(effect_args)
		
		var method := {
			"method": "_effects_callback",
			"args": [effect_args_list],
		}
		
		animation.track_insert_key(track_idx, time, method, 0.0)

func _add_sounds_track(state: GLTFState, mirrored: bool, data: PackedByteArray, animation: Animation) -> void:
	var sounds_buf := StreamPeerBuffer.new()
	sounds_buf.data_array = data
	
	var track_idx := animation.add_track(Animation.TYPE_METHOD)
	animation.track_set_path(track_idx, sequence_node_name)
	animation.track_set_interpolation_type(track_idx, Animation.INTERPOLATION_NEAREST)
	
	var frame_count := sounds_buf.get_u32()
	for frame_idx in frame_count:
		var time := sounds_buf.get_float()
		
		var sound_args_list: Array[Dictionary]
		
		var sounds_count := sounds_buf.get_u32()
		for sounds_idx in sounds_count:
			var id := sounds_buf.get_u16()
			var node_idx := sounds_buf.get_u8()
			var flags := sounds_buf.get_u32()
			
			var gltf_node := state.get_nodes()[node_idx]
			if mirrored:
				var mirror_node := _get_mirror_node(state, gltf_node)
				if mirror_node:
					gltf_node = mirror_node
			
			sound_args_list.append({
				"id": id,
				"bone": gltf_node.original_name,
				"collision_check": bool(flags & 0x1 == 0x1),
			})
		
		var method := {
			"method": "_sounds_callback",
			"args": [sound_args_list],
		}
		
		animation.track_insert_key(track_idx, time, method, 0.0)

func _add_flags_track(state: GLTFState, data: PackedByteArray, animation: Animation) -> void:
	var flags_buf := StreamPeerBuffer.new()
	flags_buf.data_array = data
	
	var track_idx := animation.add_track(Animation.TYPE_METHOD)
	animation.track_set_path(track_idx, sequence_node_name)
	animation.track_set_interpolation_type(track_idx, Animation.INTERPOLATION_NEAREST)
	
	var frame_count := flags_buf.get_u32()
	for frame_idx in frame_count:
		var time := flags_buf.get_float()
		
		var flags := flags_buf.get_u16()
		
		var method := {
			"method": "_flags_callback",
			"args": [flags],
		}
		
		animation.track_insert_key(track_idx, time, method, 0.0)
