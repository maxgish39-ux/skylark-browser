fn main() {
    cc::Build::new()
        .cpp(true)
        .file("cpp/src/render_bridge.cpp")
        .include("cpp/include")
        .warnings(true)
        .compile("browser_render_bridge");

    println!("cargo:rerun-if-changed=cpp/src/render_bridge.cpp");
    println!("cargo:rerun-if-changed=cpp/include/render_bridge.h");
}
