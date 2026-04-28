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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Switcher");
}
