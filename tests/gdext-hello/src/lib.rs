use godot::prelude::*;

struct HelloExtension;

#[gdextension]
unsafe impl ExtensionLibrary for HelloExtension {}

#[derive(GodotClass)]
#[class(base = Node, init)]
struct HelloWeb {
    base: Base<Node>,
}

#[godot_api]
impl HelloWeb {
    #[func]
    fn greet(&self, name: GString) -> GString {
        let v: Vec<u64> = (1..=10).collect();
        format!("hello {name} from Rust on wasm, sum={}", v.iter().sum::<u64>()).as_str().into()
    }
}
