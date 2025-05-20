mod godot_toml;

use godot::prelude::*;

struct TomlExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TomlExtension {}
