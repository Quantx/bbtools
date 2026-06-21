class_name BBProjectile extends CollisionShape3D

@export var config: BBWeaponConfig
@export var model: Node3D
@export var material: Material

@export var flare_effect: BBEffectConfigGroup
@export var flying_effects: Array[BBEffectConfigGroup]
@export var flying_effect_interval: float = 0.05
@export var thrust_effect: BBEffectConfigGroup
@export var thrust_effect_interval: float = 0.1
@export var mech_impact_effect: BBEffectConfigGroup

var _flying_effect_timer: float
var _flying_effect_index: int

var _thrust_effect_timer: float

func _ready() -> void:
	var mesh_inst := model.get_node_or_null(^"Skeleton3D/0") as MeshInstance3D
	if mesh_inst && material:
		mesh_inst.material_override = material
	
	if !Engine.is_editor_hint() && flare_effect:
		var flare_effect_args := {
			"attach_node": get_parent(),
		}
		
		BBEffectManager.spawn(flare_effect, flare_effect_args, self)

func _process(delta: float) -> void:
	if !flying_effects.is_empty():
		_flying_effect_timer += delta
		if _flying_effect_timer >= flying_effect_interval:
			_flying_effect_timer -= flying_effect_interval
			
			var flying_effect_args := {
				"attach_node": get_parent(),
				"detach_delay": 0.0
			}
			
			BBEffectManager.spawn(flying_effects[_flying_effect_index], flying_effect_args)
			
			_flying_effect_index += 1
			_flying_effect_index %= flying_effects.size()
	
	if thrust_effect:
		_thrust_effect_timer += delta
		if _thrust_effect_timer >= thrust_effect_interval:
			_thrust_effect_timer -= thrust_effect_interval
			
			var thrust_effect_args := {
				"attach_node": get_parent(),
			}
			
			BBEffectManager.spawn(thrust_effect, thrust_effect_args, self)
