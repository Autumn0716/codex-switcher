mod profiles;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            profiles::list_profiles,
            profiles::add_profile,
            profiles::switch_profile,
            profiles::rename_profile,
            profiles::remove_profile,
            profiles::get_requirements,
            profiles::list_custom_providers,
            profiles::save_custom_provider,
            profiles::remove_custom_provider,
            profiles::list_provider_profiles,
            profiles::save_provider_profile,
            profiles::activate_provider_profile,
            profiles::save_and_activate_provider_profile,
            profiles::start_codex_login,
            profiles::import_codex_auth,
            profiles::fetch_codex_provider_usage,
            profiles::open_external_url,
            profiles::fetch_available_models,
            profiles::read_claude_settings,
            profiles::write_claude_settings,
            profiles::read_codex_config,
            profiles::write_codex_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running code-switcher");
}
