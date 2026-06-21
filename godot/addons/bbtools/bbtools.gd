class_name BBTools extends Object

## This function accepts a path beginning with one of the following base directories:
## "res://proprietary/", "res://addons/". It returns a StringName of the path component immediately
## following the base directory. The path must not contian "$" character. The "$" character is used
## to identify content originating from the "res://proprietary/" directory.
static func get_content_prefix(path: String) -> StringName:
	if path.contains("$"):
		push_error("Content path contains a \"$\"")
		return &""
	
	var path_abs := path.trim_prefix("res://")
	if path_abs == path:
		# Path does not start with "res://"
		return &""
	
	var prefix := path_abs.get_slice("/", 1)
	if prefix == "":
		return &""
	
	var content_type := path_abs.get_slice("/", 0)
	if content_type == "proprietary":
		return &"$" + prefix
	elif content_type == "addons":
		return prefix
	
	push_error("Content path had an invalid base directory: \"%s\"" % content_type)
	return &""

static func get_content_path(prefix: StringName) -> String:
	var package := prefix.trim_prefix("$")
	var path := "res://addons/" if package == prefix else "res://proprietary/"
	return path.path_join(package)
