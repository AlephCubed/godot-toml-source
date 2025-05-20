mod godot_toml;

use crate::godot_toml::GodotToml;
use godot::classes::Engine;
use godot::prelude::*;

struct TomlExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TomlExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            Engine::singleton().register_singleton("TOML", &GodotToml::new_alloc());
        }
    }

    fn on_level_deinit(level: InitLevel) {
        if level == InitLevel::Scene {
            Engine::singleton().unregister_singleton("TOML");
        }
    }
}
