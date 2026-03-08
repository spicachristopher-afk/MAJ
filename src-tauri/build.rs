fn main() {
    // Tauri build (embed icon, etc.)
    tauri_build::build();

    // Embed UAC manifest (requireAdministrator) sur Windows uniquement.
    // Utilise le type RT_MANIFEST (24) qui est distinct du type RT_ICON (3)
    // utilisé par tauri-build — aucun conflit de ressources.
    #[cfg(target_os = "windows")]
    embed_resource::compile("manifest.rc", embed_resource::NONE);
}