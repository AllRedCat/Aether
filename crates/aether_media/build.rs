fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=Foundation");

        println!("cargo:rerun-if-changed=c/avfoundation_bridge.h");
        println!("cargo:rerun-if-changed=c/avfoundation_bridge.m");

        cc::Build::new()
            .file("c/avfoundation_bridge.m")
            .include("c")
            .flag("-fobjc-arc")
            .compile("avfoundation_bridge");
    }
}
