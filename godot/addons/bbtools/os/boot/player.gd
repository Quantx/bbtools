class_name BBBootPlayer extends Node2D

@export var boot: BBBoot

@export var line_thickness: float = -1.0
@export var line_antialias: bool = false

@export var strings_overrides: Dictionary[int,String]
var _strings: PackedStringArray

var time: float:
	set = set_time

func set_time(t: float) -> void:
	t = clampf(t, 0.0, boot.duration)
	if t != time:
		queue_redraw()
		time = t

func _ready() -> void:
	_strings = boot.get_strings()

func _draw_text(draw: BBBootDrawText) -> void:
	var string := String(strings_overrides.get(draw.string_idx, _strings[draw.string_idx]))
	
	var draw_length := mini(draw.string_length, string.length()) if draw.string_length >= 0 else string.length()
	var draw_position := draw.position
	var draw_color := draw.color
	
	var draw_visible := false
	for anim in draw.animations:
		var weight := anim.get_weight(time)
		if weight < 0.0:
			continue
		
		draw_visible = true
		
		if anim is BBBootAnimStart:
			pass
		elif anim is BBBootAnimPoints:
			var anim_points := anim as BBBootAnimPoints
			draw_position = draw_position.lerp(anim_points.vertices[0], weight)
		elif anim is BBBootAnimColor:
			var anim_color := anim as BBBootAnimColor
			draw_color = draw_color.lerp(anim_color.color, weight)
		elif anim is BBBootAnimColors:
			var anim_colors := anim as BBBootAnimColors
			draw_color = draw_color.lerp(anim_colors.colors[0], weight)
		elif anim is BBBootAnimText:
			draw_length = int(float(string.length()) * weight)
		else:
			push_error("Unimplmented BBBootAnim: ", anim)
	
	if !draw_visible:
		return
	
	string = string.substr(0, draw_length)
	
	draw_string(boot.font, draw_position, string, HORIZONTAL_ALIGNMENT_LEFT, -1.0, 16, draw_color)

func _draw_quad(draw: BBBootDrawQuad) -> void:
	var draw_vertices := draw.vertices.duplicate()
	var draw_colors := draw.colors.duplicate()
	assert(draw_vertices.size() == draw_colors.size())
	
	var draw_visible := false
	for anim in draw.animations:
		var weight := anim.get_weight(time)
		if weight <= 0.0:
			continue
		
		draw_visible = true
		
		if anim is BBBootAnimStart:
			pass
		elif anim is BBBootAnimPoints:
			var anim_points := anim as BBBootAnimPoints
			assert(draw_vertices.size() == anim_points.vertices.size())
			for i in draw_vertices.size():
				draw_vertices[i] = draw_vertices[i].lerp(anim_points.vertices[i], weight)
		elif anim is BBBootAnimColor:
			var anim_color := anim as BBBootAnimColor
			for i in draw_colors.size():
				draw_colors[i] = draw_colors[i].lerp(anim_color.color, weight)
		elif anim is BBBootAnimColors:
			var anim_colors := anim as BBBootAnimColors
			assert(draw_colors.size() <= anim_colors.colors.size())
			for i in draw_colors.size():
				draw_colors[i] = draw_colors[i].lerp(anim_colors.colors[i], weight)
		else:
			push_error("Unimplmented BBBootAnim: ", anim)
	
	if !draw_visible:
		return
	
	draw_polygon(draw_vertices, draw_colors)

func _draw_line(draw: BBBootDrawLine) -> void:
	var draw_start := draw.start
	var draw_end := draw.end
	var draw_color := draw.color
	
	var draw_visible := false
	for anim in draw.animations:
		var weight := anim.get_weight(time)
		if weight < 0.0:
			continue
		
		draw_visible = true
		
		if anim is BBBootAnimStart:
			pass
		elif anim is BBBootAnimPoints:
			var anim_points := anim as BBBootAnimPoints
			draw_start = draw_start.lerp(anim_points.vertices[0], weight)
			draw_end = draw_end.lerp(anim_points.vertices[1], weight)
		elif anim is BBBootAnimColor:
			var anim_color := anim as BBBootAnimColor
			draw_color = draw_color.lerp(anim_color.color, weight)
		elif anim is BBBootAnimColors:
			var anim_colors := anim as BBBootAnimColors
			draw_color = draw_color.lerp(anim_colors.colors[0], weight)
		else:
			push_error("Unimplmented BBBootAnim: ", anim)
	
	if !draw_visible:
		return
	
	draw_line(draw_start, draw_end, draw_color, line_thickness, line_antialias)

func _draw_sprite_def(draw: BBBootDrawSpriteDef) -> void:
	var frame_region := boot.get_spritesheet_region(draw.sprite_idx)
	
	var draw_start := draw.start
	var draw_end := draw.end
	var draw_color := draw.color
	
	var draw_visible := false
	for anim in draw.animations:
		var weight := anim.get_weight(time)
		if weight < 0.0:
			continue
		
		draw_visible = true
		
		if anim is BBBootAnimStart:
			pass
		elif anim is BBBootAnimPoints:
			var anim_points := anim as BBBootAnimPoints
			draw_start = draw_start.lerp(anim_points.vertices[0], weight)
			draw_end = draw_end.lerp(anim_points.vertices[1], weight)
		elif anim is BBBootAnimColor:
			var anim_color := anim as BBBootAnimColor
			draw_color = draw_color.lerp(anim_color.color, weight)
		elif anim is BBBootAnimColors:
			var anim_colors := anim as BBBootAnimColors
			draw_color = draw_color.lerp(anim_colors.colors[0], weight)
		else:
			push_error("Unimplmented BBBootAnim: ", anim)
	
	if !draw_visible:
		return
	
	draw_texture_rect_region(boot.texture, Rect2(draw_start, draw_end - draw_start).abs(), frame_region, draw_color)

func _draw_lines_def(draw: BBBootDrawLinesDef) -> void:
	var vertices := boot.linesdefs.defines[draw.lines_idx]
	
	var draw_position := draw.position
	var draw_rotation := draw.rotation
	var draw_scale := draw.scale
	var draw_color := draw.color
	
	var draw_visible := false
	for anim in draw.animations:
		var weight := anim.get_weight(time)
		if weight < 0.0:
			continue
		
		draw_visible = true
		
		if anim is BBBootAnimStart:
			pass
		elif anim is BBBootAnimPoints:
			var anim_points := anim as BBBootAnimPoints
			draw_position = draw_position.lerp(anim_points.vertices[0], weight)
		elif anim is BBBootAnimColor:
			var anim_color := anim as BBBootAnimColor
			draw_color = draw_color.lerp(anim_color.color, weight)
		elif anim is BBBootAnimColors:
			var anim_colors := anim as BBBootAnimColors
			draw_color = draw_color.lerp(anim_colors.colors[0], weight)
		elif anim is BBBootAnimScale:
			var anim_scale := anim as BBBootAnimScale
			draw_scale = lerpf(draw_scale, anim_scale.scale, weight)
		else:
			push_error("Unimplmented BBBootAnim: ", anim)
	
	if !draw_visible:
		return
	
	draw_set_transform(draw_position, draw_rotation, Vector2.ONE * draw_scale)
	draw_multiline(vertices, draw_color, line_thickness, line_antialias)
	draw_set_transform_matrix(Transform2D.IDENTITY)

func _draw() -> void:
	for draw in boot.draws:
		if draw is BBBootDrawText:
			_draw_text(draw as BBBootDrawText)
		elif draw is BBBootDrawQuad:
			_draw_quad(draw as BBBootDrawQuad)
		elif draw is BBBootDrawLine:
			_draw_line(draw as BBBootDrawLine)
		elif draw is BBBootDrawSpriteDef:
			_draw_sprite_def(draw as BBBootDrawSpriteDef)
		elif draw is BBBootDrawLinesDef:
			_draw_lines_def(draw as BBBootDrawLinesDef)
