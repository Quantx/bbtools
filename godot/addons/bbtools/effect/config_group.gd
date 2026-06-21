class_name BBEffectConfigGroup extends Resource

@export var effect_configs: Array[BBEffectConfig]

func instantiate() -> Array[BBEffect]:
	var eff_list: Array[BBEffect]
	for eff_cfg in effect_configs:
		eff_list.append_array(eff_cfg.instantiate())
	return eff_list
