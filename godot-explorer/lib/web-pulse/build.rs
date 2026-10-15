fn main() {
    println!("cargo:rerun-if-changed=src/pulse.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
        cc::Build::new()
            .file("src/pulse.c")
            .link_lib_modifier("+whole-archive")
            .compile("dcl_web_pulse");
    }
}
