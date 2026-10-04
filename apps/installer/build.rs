fn main() {
    // The IPC surface, declared: only these commands exist for the webview,
    // and the capability grants exactly these (capabilities/default.json).
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(ocinye_installer_commands())),
    )
    .expect("tauri build");
}

fn ocinye_installer_commands() -> &'static [&'static str] {
    &[
        "get_view",
        "set_lang",
        "start",
        "back",
        "choose_release",
        "continue_release",
        "choose_key",
        "test_connection",
        "continue_server",
        "trust",
        "other_server",
        "forget_server",
        "sudo",
        "cancel_sudo",
        "preflight",
        "continue_preflight",
        "continue_hardware",
        "continue_instance",
        "continue_distributions",
        "set_distributions",
        "set_endpoints",
        "check_dns",
        "continue_endpoints",
        "choose_tls_file",
        "set_tls",
        "continue_tls",
        "continue_admin",
        "change",
        "install",
        "stop",
        "reconnect",
        "resume",
        "remove_incomplete",
        "recheck",
        "reveal_credential",
        "acknowledge_credential",
        "show_credential",
        "show_receipt",
        "save_receipt",
        "open_ocinye",
    ]
}
