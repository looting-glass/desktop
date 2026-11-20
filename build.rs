use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("dev.lootingglass").qml_files([
            "qml/Main.qml",
            "qml/Settings.qml",
            "qml/ManualCheck.qml",
            "qml/FormPage.qml",
            "qml/FormLabel.qml",
            "qml/AccentTint.qml",
        ]),
    )
    .files(["src/bridge.rs"])
    // Bundled icons: the app icon plus fallbacks for theme icons missing outside KDE.
    .qrc("qml/icons/icons.qrc")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Widgets");
    // Target OS, not host OS (build scripts run on the host).
    let linux = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux");
    // SAFETY: only adds our bootstrap source and platform defines (plus the LayerShellQt include
    // path on Linux).
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.file("cpp/main.cpp");
            if linux {
                cc.include("/usr/include").define("LOOTINGGLASS_LAYER_SHELL", None);
            } else {
                // MSVC reads sources as the ANSI code page unless told otherwise (… in strings).
                cc.define("LOOTINGGLASS_QT_TRAY", None).flag_if_supported("/utf-8");
            }
        })
    };
    builder.build();
    if linux {
        println!("cargo:rustc-link-lib=LayerShellQtInterface");
    }
    println!("cargo:rerun-if-changed=cpp/main.cpp");
    println!("cargo:rerun-if-changed=qml/icons");
}
