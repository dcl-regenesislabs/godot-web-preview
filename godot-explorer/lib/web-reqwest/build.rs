fn main() {
    println!("cargo:rerun-if-changed=src/fetch.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
        cc::Build::new()
            .file("src/fetch.c")
            // Nothing references the EM_JS definitions directly; keep them in the side module.
            .link_lib_modifier("+whole-archive")
            .compile("dcl_web_fetch");
    }
}
