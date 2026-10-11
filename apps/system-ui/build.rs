fn main() {
    // The IPC surface, declared: only these commands exist for the webview,
    // and the capability grants exactly these (capabilities/default.json).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "session",
            "system",
            "binding",
            "surface",
            "open_workspace",
        ]),
    ))
    .expect("tauri build");
}
