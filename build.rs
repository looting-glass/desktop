use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("dev.lootlens").qml_files(["qml/Main.qml", "qml/Settings.qml"]),
    )
    .files(["src/bridge.rs"])
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Widgets");
    // Target OS, not host OS (build scripts run on the host).
    let linux = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux");
    // SAFETY: only adds our bootstrap source and, on Linux, the LayerShellQt include path.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.file("cpp/main.cpp");
            if linux {
                cc.include("/usr/include").define("LOOTLENS_LAYER_SHELL", None);
            }
        })
    };
    builder.build();
    if linux {
        println!("cargo:rustc-link-lib=LayerShellQtInterface");
    }
    println!("cargo:rerun-if-changed=cpp/main.cpp");
}
