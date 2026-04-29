use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const VALID_NAME_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
const PROVIDER_BRANDS: [&str; 3] = ["claude", "codex", "gemini"];

type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug)]
pub struct CswPaths {
    auth_file: PathBuf,
    profiles_dir: PathBuf,
    config_file: PathBuf,
    custom_root: PathBuf,
}

impl CswPaths {
    fn production() -> AppResult<Self> {
        let home = dirs::home_dir().ok_or(AppError::HomeDirectoryUnavailable)?;
        Ok(Self::new(
            home.join(".codex/auth.json"),
            home.join(".csw/profiles"),
            home.join(".csw/config.json"),
            home.join(".code-switcher"),
        ))
    }

    fn new(
        auth_file: PathBuf,
        profiles_dir: PathBuf,
        config_file: PathBuf,
        custom_root: PathBuf,
    ) -> Self {
        Self {
            auth_file,
            profiles_dir,
            config_file,
            custom_root,
        }
    }

    fn profile_path(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(format!("{name}.json"))
    }

    fn custom_providers_file(&self) -> PathBuf {
        self.custom_root.join("custom-providers.json")
    }

    fn csw_root(&self) -> PathBuf {
        self.config_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }

    fn custom_provider_dir(&self, name: &str) -> PathBuf {
        self.custom_root
            .join("providers")
            .join(sanitize_path_segment(name))
    }

    fn default_custom_auth_path(&self, name: &str) -> PathBuf {
        self.custom_provider_dir(name).join("auth.json")
    }

    fn default_custom_config_path(&self, name: &str) -> PathBuf {
        self.custom_provider_dir(name).join("config.toml")
    }

    fn provider_profiles_dir(&self, brand: &str) -> PathBuf {
        self.config_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("config")
            .join(brand)
    }

    fn codex_config_file(&self) -> PathBuf {
        self.auth_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("config.toml")
    }

    fn claude_settings_file(&self) -> PathBuf {
        self.config_file
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."))
            .join(".claude")
            .join("settings.json")
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Could not find the user home directory")]
    HomeDirectoryUnavailable,
    #[error("{0}")]
    Message(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Invalid JSON at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    name: String,
    email: String,
    plan: String,
    account_id_hash: String,
    is_active: bool,
    is_current_auth: bool,
    id_token_expired: bool,
    access_token_expired: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementsInfo {
    auth_path: String,
    profiles_path: String,
    config_path: String,
    platform: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderInput {
    name: String,
    note: String,
    website: String,
    api_key: String,
    api_base: String,
    model: String,
    auth_path: Option<String>,
    config_path: Option<String>,
    #[serde(default)]
    auth_json: Option<String>,
    #[serde(default)]
    config_toml: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CustomProviderRecord {
    name: String,
    note: String,
    website: String,
    api_key: String,
    api_base: String,
    model: String,
    auth_path: String,
    config_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderInfo {
    name: String,
    note: String,
    website: String,
    api_base: String,
    model: String,
    auth_path: String,
    config_path: String,
    has_api_key: bool,
    api_key_preview: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfilesState {
    profiles: Vec<Value>,
    active_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexLoginLaunch {
    auth_path: String,
    previous_modified_at: Option<u64>,
    auth_url: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexUsageWindow {
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    reset_after_seconds: Option<u64>,
    reset_at: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexUsageInfo {
    account_email: Option<String>,
    account_plan: Option<String>,
    account_id_hash: Option<String>,
    five_hour: Option<CodexUsageWindow>,
    weekly: Option<CodexUsageWindow>,
    error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CustomProvidersStore {
    providers: Vec<CustomProviderRecord>,
}

#[tauri::command]
pub fn list_profiles() -> AppResult<Vec<ProfileInfo>> {
    list_profiles_with_paths(&CswPaths::production()?)
}

#[tauri::command]
pub fn add_profile(name: String) -> AppResult<Vec<ProfileInfo>> {
    let paths = CswPaths::production()?;
    add_profile_with_paths(&paths, &name)?;
    list_profiles_with_paths(&paths)
}

#[tauri::command]
pub fn switch_profile(name: String) -> AppResult<Vec<ProfileInfo>> {
    let paths = CswPaths::production()?;
    switch_profile_with_paths(&paths, &name)?;
    list_profiles_with_paths(&paths)
}

#[tauri::command]
pub fn rename_profile(old_name: String, new_name: String) -> AppResult<Vec<ProfileInfo>> {
    let paths = CswPaths::production()?;
    rename_profile_with_paths(&paths, &old_name, &new_name)?;
    list_profiles_with_paths(&paths)
}

#[tauri::command]
pub fn remove_profile(name: String) -> AppResult<Vec<ProfileInfo>> {
    let paths = CswPaths::production()?;
    remove_profile_with_paths(&paths, &name)?;
    list_profiles_with_paths(&paths)
}

#[tauri::command]
pub fn list_custom_providers() -> AppResult<Vec<CustomProviderInfo>> {
    list_custom_providers_with_paths(&CswPaths::production()?)
}

#[tauri::command]
pub fn save_custom_provider(input: CustomProviderInput) -> AppResult<Vec<CustomProviderInfo>> {
    save_custom_provider_with_paths(&CswPaths::production()?, input)
}

#[tauri::command]
pub fn remove_custom_provider(name: String) -> AppResult<Vec<CustomProviderInfo>> {
    let paths = CswPaths::production()?;
    remove_custom_provider_with_paths(&paths, &name)?;
    list_custom_providers_with_paths(&paths)
}

#[tauri::command]
pub fn list_provider_profiles(brand: String) -> AppResult<ProviderProfilesState> {
    list_provider_profiles_with_paths(&CswPaths::production()?, &brand)
}

#[tauri::command]
pub fn save_provider_profile(brand: String, profile: Value) -> AppResult<ProviderProfilesState> {
    save_provider_profile_with_paths(&CswPaths::production()?, &brand, profile)
}

#[tauri::command]
pub fn activate_provider_profile(brand: String, id: String) -> AppResult<ProviderProfilesState> {
    activate_provider_profile_with_paths(&CswPaths::production()?, &brand, &id)
}

#[tauri::command]
pub fn save_and_activate_provider_profile(
    brand: String,
    profile: Value,
) -> AppResult<ProviderProfilesState> {
    save_and_activate_provider_profile_with_paths(&CswPaths::production()?, &brand, profile)
}

#[tauri::command]
pub fn start_codex_login() -> AppResult<CodexLoginLaunch> {
    let paths = CswPaths::production()?;
    let previous_modified_at = modified_seconds(&paths.auth_file)?;
    let login = launch_codex_login()?;
    Ok(CodexLoginLaunch {
        auth_path: paths.auth_file.display().to_string(),
        previous_modified_at,
        auth_url: login.auth_url,
    })
}

#[tauri::command]
pub fn import_codex_auth(profile: Value, require_modified_after: Option<u64>) -> AppResult<Value> {
    import_codex_auth_with_paths(&CswPaths::production()?, profile, require_modified_after)
}

#[tauri::command]
pub fn fetch_codex_provider_usage(profile: Value) -> AppResult<CodexUsageInfo> {
    fetch_codex_provider_usage_with(profile)
}

#[tauri::command]
pub fn open_external_url(url: String) -> AppResult<()> {
    open_external_url_with(&url)
}

#[tauri::command]
pub fn get_requirements() -> AppResult<RequirementsInfo> {
    let paths = CswPaths::production()?;
    Ok(RequirementsInfo {
        auth_path: paths.auth_file.display().to_string(),
        profiles_path: paths.profiles_dir.display().to_string(),
        config_path: paths.config_file.display().to_string(),
        platform: std::env::consts::OS.to_string(),
    })
}

fn list_profiles_with_paths(paths: &CswPaths) -> AppResult<Vec<ProfileInfo>> {
    if !paths.profiles_dir.exists() {
        return Ok(Vec::new());
    }

    let active = active_profile(paths);
    let current_account_id = read_json_optional(&paths.auth_file)?
        .as_ref()
        .map(get_account_id)
        .unwrap_or_default();

    let mut profile_paths = fs::read_dir(&paths.profiles_dir)
        .map_err(|source| AppError::Io {
            path: paths.profiles_dir.clone(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|name| !name.starts_with("__"))
        })
        .collect::<Vec<_>>();

    profile_paths.sort();

    Ok(profile_paths
        .into_iter()
        .filter_map(|path| {
            let name = path.file_stem()?.to_str()?.to_string();
            Some(profile_info_from_path(
                &path,
                &name,
                active.as_deref(),
                &current_account_id,
            ))
        })
        .collect())
}

fn add_profile_with_paths(paths: &CswPaths, name: &str) -> AppResult<ProfileInfo> {
    let name = validate_name(name)?;
    if !paths.auth_file.exists() {
        return Err(AppError::Message(format!(
            "{} not found. Run codex login first.",
            paths.auth_file.display()
        )));
    }

    let current_bytes = fs::read(&paths.auth_file).map_err(|source| AppError::Io {
        path: paths.auth_file.clone(),
        source,
    })?;
    let current: Value =
        serde_json::from_slice(&current_bytes).map_err(|source| AppError::Json {
            path: paths.auth_file.clone(),
            source,
        })?;
    let current_account_id = get_account_id(&current);

    if let Some(duplicate) = find_duplicate_account(paths, &current_account_id, name)? {
        return Err(AppError::Message(format!(
            "Same account already saved as '{duplicate}'"
        )));
    }

    let target = paths.profile_path(name);
    if target.exists() {
        let existing = read_json(&target)?;
        if get_account_id(&existing) != current_account_id {
            return Err(AppError::Message(format!(
                "Profile '{name}' already exists with a different account."
            )));
        }
    }

    write_atomic(&target, &current_bytes)?;
    update_config_active(paths, name)?;

    Ok(profile_info_from_value(
        name,
        &current,
        Some(name),
        &current_account_id,
    ))
}

fn switch_profile_with_paths(paths: &CswPaths, name: &str) -> AppResult<ProfileInfo> {
    let name = validate_name(name)?;
    let target = paths.profile_path(name);
    if !target.exists() {
        return Err(AppError::Message(format!("Profile '{name}' not found.")));
    }

    update_active_profile(paths)?;

    if paths.auth_file.exists() {
        let current_bytes = fs::read(&paths.auth_file).map_err(|source| AppError::Io {
            path: paths.auth_file.clone(),
            source,
        })?;
        write_atomic(&paths.profile_path("__current_backup"), &current_bytes)?;
    }

    let target_bytes = fs::read(&target).map_err(|source| AppError::Io {
        path: target.clone(),
        source,
    })?;
    write_atomic(&paths.auth_file, &target_bytes)?;
    update_config_active(paths, name)?;

    let target_value: Value =
        serde_json::from_slice(&target_bytes).map_err(|source| AppError::Json {
            path: target,
            source,
        })?;
    let account_id = get_account_id(&target_value);
    Ok(profile_info_from_value(
        name,
        &target_value,
        Some(name),
        &account_id,
    ))
}

fn rename_profile_with_paths(
    paths: &CswPaths,
    old_name: &str,
    new_name: &str,
) -> AppResult<ProfileInfo> {
    let old_name = validate_name(old_name)?;
    let new_name = validate_name(new_name)?;
    let old_path = paths.profile_path(old_name);
    let new_path = paths.profile_path(new_name);

    if !old_path.exists() {
        return Err(AppError::Message(format!(
            "Profile '{old_name}' not found."
        )));
    }
    if new_path.exists() {
        return Err(AppError::Message(format!(
            "Profile '{new_name}' already exists."
        )));
    }

    fs::rename(&old_path, &new_path).map_err(|source| AppError::Io {
        path: old_path.clone(),
        source,
    })?;

    if active_profile(paths).as_deref() == Some(old_name) {
        update_config_active(paths, new_name)?;
    }

    let current_account_id = read_json_optional(&paths.auth_file)?
        .as_ref()
        .map(get_account_id)
        .unwrap_or_default();
    Ok(profile_info_from_path(
        &new_path,
        new_name,
        active_profile(paths).as_deref(),
        &current_account_id,
    ))
}

fn remove_profile_with_paths(paths: &CswPaths, name: &str) -> AppResult<()> {
    let name = validate_name(name)?;
    let target = paths.profile_path(name);

    if !target.exists() {
        return Err(AppError::Message(format!("Profile '{name}' not found.")));
    }

    fs::remove_file(&target).map_err(|source| AppError::Io {
        path: target,
        source,
    })?;

    if active_profile(paths).as_deref() == Some(name) {
        update_config_active(paths, "")?;
    }

    Ok(())
}

fn list_provider_profiles_with_paths(
    paths: &CswPaths,
    brand: &str,
) -> AppResult<ProviderProfilesState> {
    let brand = validate_provider_brand(brand)?;
    ensure_csw_layout(paths)?;
    read_provider_profiles(paths, brand)
}

fn save_provider_profile_with_paths(
    paths: &CswPaths,
    brand: &str,
    profile: Value,
) -> AppResult<ProviderProfilesState> {
    let brand = validate_provider_brand(brand)?;
    ensure_csw_layout(paths)?;
    let id = provider_profile_path_id(&profile)?;
    write_json_atomic(
        &paths
            .provider_profiles_dir(brand)
            .join(format!("{id}.json")),
        &profile,
    )?;
    read_provider_profiles(paths, brand)
}

fn save_and_activate_provider_profile_with_paths(
    paths: &CswPaths,
    brand: &str,
    profile: Value,
) -> AppResult<ProviderProfilesState> {
    let result = (|| {
        let brand = validate_provider_brand(brand)?;
        let id = provider_profile_path_id(&profile)?;
        save_provider_profile_with_paths(paths, brand, profile)?;
        activate_provider_profile_with_paths(paths, brand, &id)
    })();

    if let Err(error) = &result {
        append_error_log(paths, "save_and_activate_provider_profile", error);
    }

    result
}

fn activate_provider_profile_with_paths(
    paths: &CswPaths,
    brand: &str,
    id: &str,
) -> AppResult<ProviderProfilesState> {
    let brand = validate_provider_brand(brand)?;
    ensure_csw_layout(paths)?;
    let selected_id = sanitize_required_path_segment("Provider profile id", id)?;
    let dir = paths.provider_profiles_dir(brand);
    let selected_path = dir.join(format!("{selected_id}.json"));
    if !selected_path.exists() {
        return Err(AppError::Message(format!(
            "Provider profile '{id}' not found."
        )));
    }
    let selected_profile = read_json(&selected_path)?;
    if brand == "claude" {
        apply_claude_provider_settings(paths, &selected_profile)?;
    } else if brand == "codex" {
        apply_codex_provider_runtime_files(paths, &selected_profile)?;
    }

    for path in provider_profile_paths(&dir)? {
        let is_selected = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == selected_id);
        let mut profile = match read_json(&path)? {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        if profile.get("isActive").and_then(Value::as_bool) == Some(is_selected) {
            continue;
        }
        profile.insert("isActive".to_string(), Value::Bool(is_selected));
        write_json_atomic(&path, &Value::Object(profile))?;
    }

    read_provider_profiles(paths, brand)
}

fn apply_claude_provider_settings(paths: &CswPaths, profile: &Value) -> AppResult<()> {
    let settings_path = paths.claude_settings_file();
    let mut settings = match read_json_optional(&settings_path)? {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    let mut env = settings
        .remove("env")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();

    for key in CLAUDE_PROVIDER_ENV_KEYS {
        env.remove(*key);
    }

    let use_1m_context = profile_bool(profile, "use1MContext");
    insert_env_bool(
        &mut env,
        "CLUADE_CODE_EXPERIMENTAL_AGENT_TEAMS",
        profile_bool(profile, "teammatesMode"),
    );
    insert_env_bool(
        &mut env,
        "ENABLE_TOOL_SEARCH",
        profile_bool(profile, "enableToolSearch"),
    );
    insert_env_model(
        &mut env,
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        profile_string(profile, "opusModel"),
        use_1m_context,
    );
    insert_env_string(
        &mut env,
        "ANTHROPIC_BASE_URL",
        profile_string(profile, "baseUrl"),
    );
    insert_env_string(
        &mut env,
        "ANTHROPIC_AUTH_TOKEN",
        profile_string(profile, "apiKey"),
    );
    insert_env_model(
        &mut env,
        "ANTHROPIC_MODEL",
        profile_string(profile, "mainModel"),
        use_1m_context,
    );
    insert_env_model(
        &mut env,
        "ANTHROPIC_REASONING_MODEL",
        profile_string(profile, "reasoningModel"),
        use_1m_context,
    );
    insert_env_model(
        &mut env,
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        profile_string(profile, "haikuModel"),
        use_1m_context,
    );
    insert_env_model(
        &mut env,
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        profile_string(profile, "sonnetModel"),
        use_1m_context,
    );
    insert_env_bool(
        &mut env,
        "HIDE_AI_SIGNATURE",
        profile_bool(profile, "hideAiSignature"),
    );
    insert_env_bool(
        &mut env,
        "HIGH_INTENSITY_THINKING",
        profile_bool(profile, "highIntensityThinking"),
    );
    insert_env_bool(
        &mut env,
        "DISABLE_AUTO_UPGRADE",
        profile_bool(profile, "disableAutoUpgrade"),
    );

    settings.insert("env".to_string(), Value::Object(env));
    write_json_atomic(&settings_path, &Value::Object(settings))
}

const CLAUDE_PROVIDER_ENV_KEYS: &[&str] = &[
    "CLUADE_CODE_EXPERIMENTAL_AGENT_TEAMS",
    "ENABLE_TOOL_SEARCH",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_REASONING_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "HIDE_AI_SIGNATURE",
    "HIGH_INTENSITY_THINKING",
    "DISABLE_AUTO_UPGRADE",
];

fn profile_bool(profile: &Value, key: &str) -> bool {
    profile.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn profile_string<'a>(profile: &'a Value, key: &str) -> Option<&'a str> {
    profile
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn insert_env_bool(env: &mut Map<String, Value>, key: &str, enabled: bool) {
    if enabled {
        env.insert(key.to_string(), Value::String("true".to_string()));
    }
}

fn insert_env_string(env: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        env.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn insert_env_model(env: &mut Map<String, Value>, key: &str, value: Option<&str>, use_1m: bool) {
    if let Some(value) = value {
        env.insert(
            key.to_string(),
            Value::String(append_1m_model(value, use_1m)),
        );
    }
}

fn append_1m_model(model: &str, use_1m: bool) -> String {
    if use_1m && !model.ends_with("[1m]") {
        format!("{model}[1m]")
    } else {
        model.to_string()
    }
}

fn apply_codex_provider_runtime_files(paths: &CswPaths, profile: &Value) -> AppResult<()> {
    if let Some(auth_json) = profile
        .get("authJson")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let auth = serde_json::from_str::<Value>(auth_json).map_err(|source| AppError::Json {
            path: paths.auth_file.clone(),
            source,
        })?;
        if codex_auth_has_runtime_credentials(&auth) {
            let bytes = serde_json::to_vec_pretty(&auth).map_err(|error| {
                AppError::Message(format!(
                    "Could not serialize Codex auth for {}: {error}",
                    paths.auth_file.display()
                ))
            })?;
            write_atomic(&paths.auth_file, &bytes)?;
        }
    }

    if let Some(config_toml) = profile
        .get("configToml")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let config_path = paths.codex_config_file();
        validate_config_toml(&config_path, config_toml)?;
        write_atomic(&config_path, config_toml.as_bytes())?;
    }

    Ok(())
}

fn import_codex_auth_with_paths(
    paths: &CswPaths,
    profile: Value,
    require_modified_after: Option<u64>,
) -> AppResult<Value> {
    if let Some(required_modified_at) = require_modified_after {
        let Some(actual_modified_at) = modified_seconds(&paths.auth_file)? else {
            return Err(AppError::Message(format!(
                "{} not found. Finish Codex login first.",
                paths.auth_file.display()
            )));
        };
        if actual_modified_at <= required_modified_at {
            return Err(AppError::Message(format!(
                "{} has not changed since Codex login started.",
                paths.auth_file.display()
            )));
        }
    }

    let auth = read_json(&paths.auth_file)?;
    validate_codex_auth_value(&paths.auth_file, &auth)?;
    let auth_json = serde_json::to_string_pretty(&auth).map_err(|error| {
        AppError::Message(format!(
            "Could not serialize {}: {error}",
            paths.auth_file.display()
        ))
    })?;

    let mut profile = match profile {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    profile.insert("authJson".to_string(), Value::String(auth_json));
    profile.insert(
        "authMode".to_string(),
        Value::String(codex_auth_mode(&auth).to_string()),
    );

    if let Some(api_key) = auth.get("OPENAI_API_KEY").and_then(Value::as_str) {
        if !api_key.trim().is_empty() {
            profile.insert("apiKey".to_string(), Value::String(api_key.to_string()));
        }
    }

    Ok(Value::Object(profile))
}

fn fetch_codex_provider_usage_with(profile: Value) -> AppResult<CodexUsageInfo> {
    let Some(auth) = codex_auth_for_usage(&profile) else {
        return Ok(CodexUsageInfo {
            error: Some("missing_auth".to_string()),
            ..CodexUsageInfo::default()
        });
    };
    let mut info = codex_usage_identity(&auth);
    let access_token = auth
        .get("tokens")
        .and_then(|tokens| tokens.get("access_token"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let account_id = get_account_id(&auth);

    if access_token.trim().is_empty() || account_id.trim().is_empty() {
        info.error = Some("missing_auth".to_string());
        return Ok(info);
    }

    if token_expired(&auth, "access_token") {
        info.error = Some("token_expired".to_string());
        return Ok(info);
    }

    match fetch_codex_usage_json(access_token, &account_id)? {
        UsageFetchResult::Ok(usage) => Ok(codex_usage_info_from_value(info, &usage)),
        UsageFetchResult::TokenExpired => {
            info.error = Some("token_expired".to_string());
            Ok(info)
        }
        UsageFetchResult::Forbidden => {
            info.error = Some("forbidden".to_string());
            Ok(info)
        }
        UsageFetchResult::Unavailable => {
            info.error = Some("unavailable".to_string());
            Ok(info)
        }
    }
}

fn codex_auth_for_usage(profile: &Value) -> Option<Value> {
    let profile_auth = codex_auth_from_provider_profile(profile);
    if profile_auth.as_ref().is_some_and(|auth| {
        codex_auth_has_usage_credentials(auth) && !token_expired(auth, "access_token")
    }) {
        return profile_auth;
    }

    None
}

fn codex_auth_from_provider_profile(profile: &Value) -> Option<Value> {
    if let Some(auth_json) = profile.get("authJson").and_then(Value::as_str) {
        serde_json::from_str(auth_json).ok()
    } else if profile.get("tokens").is_some() {
        Some(profile.clone())
    } else {
        None
    }
}

fn codex_auth_has_usage_credentials(auth: &Value) -> bool {
    let access_token = auth
        .get("tokens")
        .and_then(|tokens| tokens.get("access_token"))
        .and_then(Value::as_str)
        .unwrap_or("");
    !access_token.trim().is_empty() && !get_account_id(auth).trim().is_empty()
}

fn codex_auth_has_runtime_credentials(auth: &Value) -> bool {
    auth.get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .is_some_and(|key| !key.trim().is_empty())
        || auth
            .get("tokens")
            .and_then(Value::as_object)
            .is_some_and(|tokens| {
                ["access_token", "refresh_token", "id_token", "account_id"]
                    .iter()
                    .any(|key| {
                        tokens
                            .get(*key)
                            .and_then(Value::as_str)
                            .is_some_and(|token| !token.trim().is_empty())
                    })
            })
}

fn codex_usage_identity(auth: &Value) -> CodexUsageInfo {
    let id_payload = token_payload(auth, "id_token");
    let account_email = id_payload
        .as_ref()
        .and_then(|payload| payload.get("email"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let account_plan = id_payload
        .as_ref()
        .and_then(|payload| payload.get("https://api.openai.com/auth"))
        .and_then(|auth| auth.get("chatgpt_plan_type"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let account_id = get_account_id(auth);

    CodexUsageInfo {
        account_email,
        account_plan,
        account_id_hash: (!account_id.is_empty()).then(|| hash_account_id(&account_id)),
        ..CodexUsageInfo::default()
    }
}

enum UsageFetchResult {
    Ok(Value),
    TokenExpired,
    Forbidden,
    Unavailable,
}

fn fetch_codex_usage_json(access_token: &str, account_id: &str) -> AppResult<UsageFetchResult> {
    let curl = curl_binary();
    let output = Command::new(&curl)
        .arg("-sS")
        .arg("--max-time")
        .arg("15")
        .arg("--connect-timeout")
        .arg("8")
        .arg("-w")
        .arg("\n%{http_code}")
        .arg("-H")
        .arg(format!("Authorization: Bearer {access_token}"))
        .arg("-H")
        .arg(format!("ChatGPT-Account-Id: {account_id}"))
        .arg("-H")
        .arg("Accept: application/json")
        .arg("-H")
        .arg("Origin: https://chatgpt.com")
        .arg("-H")
        .arg("Referer: https://chatgpt.com/")
        .arg("-H")
        .arg("User-Agent: Mozilla/5.0")
        .arg("https://chatgpt.com/backend-api/wham/usage")
        .output()
        .map_err(|source| AppError::Io { path: curl, source })?;

    if !output.status.success() || output.stdout.is_empty() {
        return Ok(UsageFetchResult::Unavailable);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let Some((body, status)) = text.trim_end().rsplit_once('\n') else {
        return Ok(UsageFetchResult::Unavailable);
    };
    let status = status.trim().parse::<u16>().unwrap_or(0);
    match status {
        200 => serde_json::from_str(body)
            .map(UsageFetchResult::Ok)
            .map_err(|error| AppError::Message(format!("Invalid usage response JSON: {error}"))),
        401 => Ok(UsageFetchResult::TokenExpired),
        403 => Ok(UsageFetchResult::Forbidden),
        _ => Ok(UsageFetchResult::Unavailable),
    }
}

fn curl_binary() -> PathBuf {
    let path = PathBuf::from("/usr/bin/curl");
    if path.is_file() {
        path
    } else {
        PathBuf::from("curl")
    }
}

fn codex_usage_info_from_value(mut info: CodexUsageInfo, usage: &Value) -> CodexUsageInfo {
    if info.account_email.is_none() {
        info.account_email = usage
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    if info.account_id_hash.is_none() {
        info.account_id_hash = usage
            .get("account_id")
            .or_else(|| usage.get("user_id"))
            .and_then(Value::as_str)
            .map(hash_account_id);
    }
    if let Some(plan_type) = usage.get("plan_type").and_then(Value::as_str) {
        info.account_plan = Some(plan_type.to_string());
    }

    let rate_limit = usage.get("rate_limit").unwrap_or(&Value::Null);
    info.five_hour = parse_codex_usage_window(rate_limit.get("primary_window"));
    info.weekly = parse_codex_usage_window(rate_limit.get("secondary_window"));
    info
}

fn parse_codex_usage_window(window: Option<&Value>) -> Option<CodexUsageWindow> {
    let window = window?;
    let used_percent = value_as_f64(window.get("used_percent")).or_else(|| {
        let limit = value_as_f64(window.get("limit"))
            .or_else(|| value_as_f64(window.get("max")))
            .or_else(|| value_as_f64(window.get("budget")))?;
        let used = value_as_f64(window.get("used"))
            .or_else(|| value_as_f64(window.get("consumed")))
            .unwrap_or(0.0);
        (limit > 0.0).then(|| used / limit * 100.0)
    });
    let remaining_percent = used_percent.map(|used| (100.0 - used).clamp(0.0, 100.0));

    Some(CodexUsageWindow {
        used_percent,
        remaining_percent,
        reset_after_seconds: value_as_u64(window.get("reset_after_seconds")),
        reset_at: value_as_u64(window.get("reset_at"))
            .or_else(|| value_as_u64(window.get("resets_at")))
            .or_else(|| value_as_u64(window.get("reset_time"))),
    })
}

fn value_as_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|value| value.parse::<f64>().ok()))
    })
}

fn value_as_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_f64().map(|value| value as u64))
            .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
    })
}

fn read_provider_profiles(paths: &CswPaths, brand: &str) -> AppResult<ProviderProfilesState> {
    let dir = paths.provider_profiles_dir(brand);
    let mut profiles = provider_profile_paths(&dir)?
        .into_iter()
        .map(|path| read_json(&path))
        .collect::<AppResult<Vec<_>>>()?;

    profiles.sort_by(|left, right| {
        provider_profile_sort_key(left).cmp(&provider_profile_sort_key(right))
    });

    let active_id = profiles
        .iter()
        .find(|profile| {
            profile
                .get("isActive")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .and_then(|profile| profile.get("id").and_then(Value::as_str))
        .map(str::to_string);

    Ok(ProviderProfilesState {
        profiles,
        active_id,
    })
}

fn provider_profile_paths(dir: &Path) -> AppResult<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = fs::read_dir(dir)
        .map_err(|source| AppError::Io {
            path: dir.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn provider_profile_sort_key(profile: &Value) -> String {
    profile
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| profile.get("id").and_then(Value::as_str))
        .unwrap_or("")
        .to_lowercase()
}

fn provider_profile_path_id(profile: &Value) -> AppResult<String> {
    let Some(id) = profile.get("id").and_then(Value::as_str) else {
        return Err(AppError::Message(
            "Provider profile id is required.".to_string(),
        ));
    };
    sanitize_required_path_segment("Provider profile id", id)
}

fn sanitize_required_path_segment(label: &str, value: &str) -> AppResult<String> {
    if value.trim().is_empty() {
        return Err(AppError::Message(format!("{label} is required.")));
    }
    Ok(sanitize_path_segment(value))
}

fn validate_provider_brand(brand: &str) -> AppResult<&str> {
    if PROVIDER_BRANDS.contains(&brand) {
        Ok(brand)
    } else {
        Err(AppError::Message(
            "Provider brand must be claude, codex, or gemini.".to_string(),
        ))
    }
}

fn validate_codex_auth_value(path: &Path, value: &Value) -> AppResult<()> {
    let has_chatgpt_tokens = value
        .get("tokens")
        .and_then(Value::as_object)
        .is_some_and(|tokens| {
            tokens
                .get("access_token")
                .and_then(Value::as_str)
                .is_some_and(|token| !token.trim().is_empty())
                && tokens
                    .get("refresh_token")
                    .and_then(Value::as_str)
                    .is_some_and(|token| !token.trim().is_empty())
        });
    let has_api_key = value
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .is_some_and(|key| !key.trim().is_empty());

    if has_chatgpt_tokens || has_api_key {
        Ok(())
    } else {
        Err(AppError::Message(format!(
            "{} does not contain Codex login tokens or OPENAI_API_KEY.",
            path.display()
        )))
    }
}

fn codex_auth_mode(value: &Value) -> &'static str {
    if let Some(mode) = value.get("auth_mode").and_then(Value::as_str) {
        if mode == "apikey" {
            return "apikey";
        }
    }

    if value.get("tokens").is_some() {
        "chatgpt"
    } else {
        "apikey"
    }
}

fn modified_seconds(path: &Path) -> AppResult<Option<u64>> {
    if !path.exists() {
        return Ok(None);
    }

    let metadata = fs::metadata(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let modified = metadata.modified().map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let seconds = modified
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Ok(Some(seconds))
}

#[derive(Debug, Default)]
struct CodexLoginDetails {
    auth_url: Option<String>,
}

fn launch_codex_login() -> AppResult<CodexLoginDetails> {
    let (mut command, command_path) = codex_login_command();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|source| AppError::Io {
        path: command_path,
        source,
    })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel::<String>();

    if let Some(stdout) = stdout {
        drain_login_output(stdout, sender.clone());
    }
    if let Some(stderr) = stderr {
        drain_login_output(stderr, sender.clone());
    }

    thread::spawn(move || {
        let _ = child.wait();
    });

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut output = String::new();
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => {
                output.push_str(&line);
                output.push('\n');
                if let Some(details) = parse_codex_login_output(&output) {
                    return Ok(details);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Err(AppError::Message(format!(
        "Could not read Codex login URL from `codex login`. Output: {}",
        strip_ansi(&output).trim()
    )))
}

fn open_external_url_with(url: &str) -> AppResult<()> {
    let url = validate_external_auth_url(url)?;

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|source| AppError::Io {
                path: PathBuf::from("open"),
                source,
            })?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|source| AppError::Io {
                path: PathBuf::from("cmd"),
                source,
            })?;
        return Ok(());
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|source| AppError::Io {
                path: PathBuf::from("xdg-open"),
                source,
            })?;
        Ok(())
    }
}

fn validate_external_auth_url(url: &str) -> AppResult<&str> {
    if url.trim() != url
        || !url.starts_with("https://auth.openai.com/")
        || url.chars().any(char::is_control)
    {
        return Err(AppError::Message(
            "Only OpenAI authentication URLs can be opened.".to_string(),
        ));
    }

    Ok(url)
}

fn codex_login_command() -> (Command, PathBuf) {
    let path_env = augmented_path_env();
    if let Some(codex) = find_codex_binary() {
        let mut command = Command::new(&codex);
        command.arg("login");
        command.env("PATH", &path_env);
        return (command, codex);
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "codex", "login"]);
        command.env("PATH", &path_env);
        return (command, PathBuf::from("cmd"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let shell = if Path::new("/bin/zsh").exists() {
            PathBuf::from("/bin/zsh")
        } else {
            PathBuf::from("/bin/sh")
        };
        let mut command = Command::new(&shell);
        command.args(["-lc", "codex login"]);
        command.env("PATH", path_env);
        (command, shell)
    }
}

fn find_codex_binary() -> Option<PathBuf> {
    find_codex_binary_with(
        env::var_os("CODEX_BIN"),
        env::var_os("PATH"),
        dirs::home_dir(),
    )
}

fn find_codex_binary_with(
    codex_bin: Option<OsString>,
    path_env: Option<OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(path) = codex_bin.map(PathBuf::from).filter(|path| path.is_file()) {
        return Some(path);
    }

    for dir in login_path_dirs(path_env, home) {
        for name in codex_binary_names() {
            let path = dir.join(name);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

fn augmented_path_env() -> OsString {
    let current = env::var_os("PATH");
    let dirs = login_path_dirs(current.clone(), dirs::home_dir());
    env::join_paths(dirs).unwrap_or_else(|_| current.unwrap_or_default())
}

fn login_path_dirs(path_env: Option<OsString>, home: Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs = path_env
        .as_deref()
        .map(env::split_paths)
        .map(Iterator::collect::<Vec<_>>)
        .unwrap_or_default();

    for path in [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
        PathBuf::from("/opt/homebrew/sbin"),
        PathBuf::from("/usr/local/sbin"),
    ] {
        push_unique_path(&mut dirs, path);
    }

    if let Some(home) = home {
        for path in [
            home.join(".npm-global/bin"),
            home.join(".local/bin"),
            home.join(".bun/bin"),
            home.join(".volta/bin"),
        ] {
            push_unique_path(&mut dirs, path);
        }
    }

    dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn codex_binary_names() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["codex.exe", "codex.cmd", "codex"]
    }

    #[cfg(not(target_os = "windows"))]
    {
        &["codex"]
    }
}

fn drain_login_output<R>(reader: R, sender: mpsc::Sender<String>)
where
    R: io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let _ = sender.send(line.trim_end().to_string());
                }
                Err(_) => break,
            }
        }
    });
}

fn parse_codex_login_output(output: &str) -> Option<CodexLoginDetails> {
    let clean = strip_ansi(output);
    let auth_url = clean
        .split_whitespace()
        .map(trim_token)
        .find(|token| {
            token.starts_with("https://") && token.contains("auth.openai.com/oauth/authorize")
        })
        .map(str::to_string);

    auth_url.map(|auth_url| CodexLoginDetails {
        auth_url: Some(auth_url),
    })
}

fn trim_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        !(ch.is_ascii_alphanumeric()
            || matches!(ch, ':' | '/' | '.' | '-' | '_' | '?' | '&' | '=' | '%'))
    })
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn ensure_csw_layout(paths: &CswPaths) -> AppResult<()> {
    let root = paths.csw_root();
    for brand in PROVIDER_BRANDS {
        let path = root.join("config").join(brand);
        fs::create_dir_all(&path).map_err(|source| AppError::Io {
            path: path.clone(),
            source,
        })?;
        ensure_default_provider_profile(paths, brand)?;
    }

    for dir in ["prompts", "diagrams", "data", "backups"] {
        let path = root.join(dir);
        fs::create_dir_all(&path).map_err(|source| AppError::Io { path, source })?;
    }

    let logs = root.join("data/logs.db");
    ensure_logs_db(&logs)?;

    Ok(())
}

fn ensure_default_provider_profile(paths: &CswPaths, brand: &str) -> AppResult<()> {
    let profile = default_provider_profile(brand)?;
    let id = provider_profile_path_id(&profile)?;
    let path = paths
        .provider_profiles_dir(brand)
        .join(format!("{id}.json"));
    if path.exists() {
        let existing = read_json(&path)?;
        if should_restore_default_provider_profile(brand, &existing) {
            write_json_atomic(&path, &profile)?;
        }
        return Ok(());
    }

    write_json_atomic(&path, &profile)
}

fn should_restore_default_provider_profile(brand: &str, profile: &Value) -> bool {
    let api_key_is_empty = profile
        .get("apiKey")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .is_empty();
    if !api_key_is_empty {
        return false;
    }

    match brand {
        "claude" => {
            profile.get("id").and_then(Value::as_str) == Some("anthropic")
                && profile.get("mainModel").and_then(Value::as_str) == Some("claude-sonnet-4-5")
                && profile.get("notes").and_then(Value::as_str)
                    == Some("Default Anthropic Claude provider.")
        }
        "codex" => {
            profile.get("id").and_then(Value::as_str) == Some("openai")
                && codex_default_profile_auth_is_empty(profile)
                && ((profile.get("authMode").and_then(Value::as_str) == Some("apikey")
                    && profile.get("modelName").and_then(Value::as_str) == Some("gpt-5.5"))
                    || profile.get("modelName").and_then(Value::as_str) == Some("gpt-5-codex"))
        }
        "gemini" => {
            profile.get("id").and_then(Value::as_str) == Some("google")
                && profile.get("baseUrl").and_then(Value::as_str)
                    == Some("https://generativelanguage.googleapis.com/v1beta")
                && profile.get("model").and_then(Value::as_str) == Some("gemini-2.5-pro")
        }
        _ => false,
    }
}

fn codex_default_profile_auth_is_empty(profile: &Value) -> bool {
    let Some(auth_json) = profile.get("authJson").and_then(Value::as_str) else {
        return true;
    };
    let Ok(auth) = serde_json::from_str::<Value>(auth_json) else {
        return true;
    };
    let has_api_key = auth
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .is_some_and(|key| !key.trim().is_empty());
    let has_token = auth
        .get("tokens")
        .and_then(Value::as_object)
        .is_some_and(|tokens| {
            ["access_token", "refresh_token", "id_token"]
                .iter()
                .any(|key| {
                    tokens
                        .get(*key)
                        .and_then(Value::as_str)
                        .is_some_and(|token| !token.trim().is_empty())
                })
        });

    !has_api_key && !has_token
}

fn default_provider_profile(brand: &str) -> AppResult<Value> {
    match brand {
        "claude" => Ok(json!({
            "id": "anthropic",
            "name": "Anthropic",
            "notes": "",
            "website": "https://api.anthropic.com",
            "apiKey": "",
            "baseUrl": "https://api.anthropic.com",
            "apiFormat": "anthropic_messages",
            "authField": "ANTHROPIC_AUTH_TOKEN",
            "mainModel": "claude-sonnet-4-6-20250514",
            "reasoningModel": "claude-sonnet-4-6-20250514",
            "haikuModel": "claude-haiku-4-5-20251001",
            "sonnetModel": "claude-sonnet-4-6-20250514",
            "opusModel": "claude-opus-4-7-20250416",
            "hideAiSignature": false,
            "teammatesMode": false,
            "enableToolSearch": true,
            "highIntensityThinking": false,
            "disableAutoUpgrade": false,
            "useSeparateTestConfig": false,
            "useSeparateProxy": false,
            "useSeparateBilling": false,
            "isActive": true
        })),
        "codex" => Ok(json!({
            "id": "openai",
            "name": "OpenAI",
            "notes": "",
            "website": "https://api.openai.com",
            "apiKey": "",
            "baseUrl": "https://api.openai.com",
            "modelName": "gpt-5.5",
            "authMode": "chatgpt",
            "authJson": "{\n  \"auth_mode\": \"chatgpt\",\n  \"OPENAI_API_KEY\": null,\n  \"tokens\": {\n    \"id_token\": \"\",\n    \"access_token\": \"\",\n    \"refresh_token\": \"\",\n    \"account_id\": \"\"\n  },\n  \"last_refresh\": \"\"\n}",
            "contextWindow": "1000000",
            "autoCompactThreshold": "900000",
            "reasoningEffort": "xhigh",
            "approvalsReviewer": "user",
            "notifyPath": "",
            "configToml": "[codex]\nmodel = \"gpt-5.5\"\nmodel_provider = \"openai\"\nmodel_context_window = 1000000\nmodel_auto_compact_token_limit = 900000\nmodel_reasoning_effort = \"xhigh\"\napprovals_reviewer = \"user\"",
            "isActive": true
        })),
        "gemini" => Ok(json!({
            "id": "google",
            "name": "Google",
            "apiKey": "",
            "baseUrl": "https://generativelanguage.googleapis.com",
            "model": "gemini-2.5-pro-preview-05-06",
            "isActive": true
        })),
        _ => Err(AppError::Message(
            "Provider brand must be claude, codex, or gemini.".to_string(),
        )),
    }
}

fn list_custom_providers_with_paths(paths: &CswPaths) -> AppResult<Vec<CustomProviderInfo>> {
    let mut providers = load_custom_providers_with_paths(paths)?
        .providers
        .into_iter()
        .map(custom_provider_info)
        .collect::<Vec<_>>();

    providers.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(providers)
}

fn save_custom_provider_with_paths(
    paths: &CswPaths,
    input: CustomProviderInput,
) -> AppResult<Vec<CustomProviderInfo>> {
    let name = validate_required("Provider name", &input.name)?;
    let api_base = validate_required("API request URL", &input.api_base)?;
    let model = validate_required("Model name", &input.model)?;
    let api_base = api_base.trim_end_matches('/').to_string();
    let mut store = load_custom_providers_with_paths(paths)?;
    let saved_api_key = store
        .providers
        .iter()
        .find(|provider| provider.name.to_lowercase() == name.to_lowercase())
        .map(|provider| provider.api_key.clone())
        .unwrap_or_default();
    let api_key = if input.api_key.trim().is_empty() {
        saved_api_key
    } else {
        input.api_key
    };

    let auth_path = path_from_optional(
        input.auth_path.as_deref(),
        paths.default_custom_auth_path(name),
    );
    let config_path = path_from_optional(
        input.config_path.as_deref(),
        paths.default_custom_config_path(name),
    );
    let auth_content = input.auth_json.unwrap_or_else(|| {
        generated_custom_auth_json(
            name,
            &input.note,
            &input.website,
            &api_key,
            &api_base,
            model,
        )
    });
    let config_content = input
        .config_toml
        .unwrap_or_else(|| generated_custom_config_toml(name, &api_base, model));

    validate_auth_json(&auth_path, &auth_content)?;
    validate_config_toml(&config_path, &config_content)?;

    backup_if_exists(&auth_path)?;
    backup_if_exists(&config_path)?;
    write_atomic(&auth_path, auth_content.as_bytes())?;
    write_atomic(&config_path, config_content.as_bytes())?;

    store
        .providers
        .retain(|provider| provider.name.to_lowercase() != name.to_lowercase());
    store.providers.push(CustomProviderRecord {
        name: name.to_string(),
        note: input.note.trim().to_string(),
        website: input.website.trim().to_string(),
        api_key,
        api_base,
        model: model.to_string(),
        auth_path: auth_path.display().to_string(),
        config_path: config_path.display().to_string(),
    });
    store
        .providers
        .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    save_custom_providers_with_paths(paths, &store)?;
    list_custom_providers_with_paths(paths)
}

fn remove_custom_provider_with_paths(paths: &CswPaths, name: &str) -> AppResult<()> {
    let name = validate_required("Provider name", name)?;
    let mut store = load_custom_providers_with_paths(paths)?;
    let original_len = store.providers.len();
    store
        .providers
        .retain(|provider| provider.name.to_lowercase() != name.to_lowercase());

    if store.providers.len() == original_len {
        return Err(AppError::Message(format!(
            "Custom provider '{name}' not found."
        )));
    }

    save_custom_providers_with_paths(paths, &store)
}

fn load_custom_providers_with_paths(paths: &CswPaths) -> AppResult<CustomProvidersStore> {
    let path = paths.custom_providers_file();
    if !path.exists() {
        return Ok(CustomProvidersStore::default());
    }

    let value = read_json(&path)?;
    serde_json::from_value(value).map_err(|source| AppError::Json { path, source })
}

fn save_custom_providers_with_paths(
    paths: &CswPaths,
    store: &CustomProvidersStore,
) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(store).map_err(|error| {
        AppError::Message(format!("Could not serialize custom providers: {error}"))
    })?;
    write_atomic(&paths.custom_providers_file(), &bytes)
}

fn custom_provider_info(record: CustomProviderRecord) -> CustomProviderInfo {
    CustomProviderInfo {
        name: record.name,
        note: record.note,
        website: record.website,
        api_base: record.api_base,
        model: record.model,
        auth_path: record.auth_path,
        config_path: record.config_path,
        has_api_key: !record.api_key.trim().is_empty(),
        api_key_preview: preview_secret(&record.api_key),
    }
}

fn validate_required<'a>(label: &str, value: &'a str) -> AppResult<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Message(format!("{label} is required.")));
    }
    Ok(trimmed)
}

fn path_from_optional(value: Option<&str>, fallback: PathBuf) -> PathBuf {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(fallback)
}

fn generated_custom_auth_json(
    name: &str,
    note: &str,
    website: &str,
    api_key: &str,
    api_base: &str,
    model: &str,
) -> String {
    let openai_key = if api_key.trim().is_empty() {
        Value::Null
    } else {
        Value::String(api_key.to_string())
    };
    let value = json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": openai_key,
        "custom_provider": {
            "name": name,
            "note": note.trim(),
            "website": website.trim(),
            "api_base": api_base,
            "model": model
        }
    });

    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

fn generated_custom_config_toml(name: &str, api_base: &str, model: &str) -> String {
    let provider_key = sanitize_path_segment(name);
    format!(
        "model = \"{}\"\nmodel_provider = \"{}\"\n\n[model_providers.{}]\nname = \"{}\"\nbase_url = \"{}\"\nenv_key = \"OPENAI_API_KEY\"\nwire_api = \"responses\"\n",
        toml_escape(model),
        toml_escape(&provider_key),
        provider_key,
        toml_escape(name),
        toml_escape(api_base),
    )
}

fn validate_auth_json(path: &Path, content: &str) -> AppResult<()> {
    serde_json::from_str::<Value>(content)
        .map(|_| ())
        .map_err(|source| AppError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn validate_config_toml(path: &Path, content: &str) -> AppResult<()> {
    toml::from_str::<toml::Value>(content)
        .map(|_| ())
        .map_err(|source| {
            AppError::Message(format!("Invalid TOML at {}: {source}", path.display()))
        })
}

fn sanitize_path_segment(name: &str) -> String {
    let segment = name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if segment.is_empty() {
        "provider".to_string()
    } else {
        segment
    }
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn preview_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= 4 {
        return "****".to_string();
    }

    let prefix = chars.iter().take(4).collect::<String>();
    let suffix = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn backup_if_exists(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Ok(());
    }

    let timestamp = now_seconds();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bak");
    let backup = path.with_extension(format!("{extension}.bak.{timestamp}"));
    fs::copy(path, &backup).map_err(|source| AppError::Io {
        path: backup,
        source,
    })?;
    Ok(())
}

fn validate_name(name: &str) -> AppResult<&str> {
    if name.is_empty() {
        return Err(AppError::Message(
            "Profile name cannot be empty".to_string(),
        ));
    }

    let invalid = name
        .chars()
        .filter(|ch| !VALID_NAME_CHARS.contains(*ch))
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return Err(AppError::Message(
            "Profile names can only use letters, digits, dash, and underscore.".to_string(),
        ));
    }

    if name.starts_with("__") {
        return Err(AppError::Message(
            "Names starting with '__' are reserved.".to_string(),
        ));
    }

    Ok(name)
}

fn profile_info_from_path(
    path: &Path,
    name: &str,
    active: Option<&str>,
    current_account_id: &str,
) -> ProfileInfo {
    match read_json(path) {
        Ok(value) => profile_info_from_value(name, &value, active, current_account_id),
        Err(_) => ProfileInfo {
            name: name.to_string(),
            email: "?".to_string(),
            plan: "?".to_string(),
            account_id_hash: String::new(),
            is_active: active == Some(name),
            is_current_auth: false,
            id_token_expired: false,
            access_token_expired: false,
        },
    }
}

fn profile_info_from_value(
    name: &str,
    value: &Value,
    active: Option<&str>,
    current_account_id: &str,
) -> ProfileInfo {
    let account_id = get_account_id(value);
    let id_payload = token_payload(value, "id_token");
    let email = id_payload
        .as_ref()
        .and_then(|payload| payload.get("email"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let plan = id_payload
        .as_ref()
        .and_then(|payload| payload.get("https://api.openai.com/auth"))
        .and_then(|auth| auth.get("chatgpt_plan_type"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();

    ProfileInfo {
        name: name.to_string(),
        email,
        plan,
        account_id_hash: hash_account_id(&account_id),
        is_active: active == Some(name),
        is_current_auth: !account_id.is_empty() && account_id == current_account_id,
        id_token_expired: token_expired(value, "id_token"),
        access_token_expired: token_expired(value, "access_token"),
    }
}

fn token_payload(value: &Value, token_name: &str) -> Option<Value> {
    value
        .get("tokens")?
        .get(token_name)?
        .as_str()
        .and_then(|token| jwt_payload(token).ok())
}

fn token_expired(value: &Value, token_name: &str) -> bool {
    let Some(exp) = token_payload(value, token_name)
        .and_then(|payload| payload.get("exp").cloned())
        .and_then(|exp| {
            exp.as_u64()
                .or_else(|| exp.as_f64().map(|value| value as u64))
        })
    else {
        return false;
    };

    exp != 0 && now_seconds() > exp
}

fn jwt_payload(token: &str) -> AppResult<Value> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| AppError::Message("Invalid JWT: missing payload.".to_string()))?;
    let mut padded = payload.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }

    let decoded = URL_SAFE
        .decode(padded)
        .map_err(|error| AppError::Message(format!("Invalid JWT payload: {error}")))?;
    serde_json::from_slice(&decoded)
        .map_err(|error| AppError::Message(format!("Invalid JWT payload: {error}")))
}

fn hash_account_id(account_id: &str) -> String {
    if account_id.is_empty() {
        return String::new();
    }

    let digest = Sha256::digest(account_id.as_bytes());
    let short = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{short}")
}

fn get_account_id(value: &Value) -> String {
    value
        .get("tokens")
        .and_then(|tokens| tokens.get("account_id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn find_duplicate_account(
    paths: &CswPaths,
    account_id: &str,
    exclude_name: &str,
) -> AppResult<Option<String>> {
    if account_id.is_empty() || !paths.profiles_dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(&paths.profiles_dir).map_err(|source| AppError::Io {
        path: paths.profiles_dir.clone(),
        source,
    })? {
        let path = entry
            .map_err(|source| AppError::Io {
                path: paths.profiles_dir.clone(),
                source,
            })?
            .path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }

        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if name.starts_with("__") || name == exclude_name {
            continue;
        }

        let Ok(value) = read_json(&path) else {
            continue;
        };
        if get_account_id(&value) == account_id {
            return Ok(Some(name.to_string()));
        }
    }

    Ok(None)
}

fn update_active_profile(paths: &CswPaths) -> AppResult<()> {
    let Some(active) = active_profile(paths) else {
        return Ok(());
    };
    if active.is_empty() || !paths.auth_file.exists() {
        return Ok(());
    }

    let target = paths.profile_path(&active);
    if !target.exists() {
        return Ok(());
    }

    let Some(current) = read_json_optional(&paths.auth_file)? else {
        return Ok(());
    };
    let Some(saved) = read_json_optional(&target)? else {
        return Ok(());
    };
    let current_account_id = get_account_id(&current);
    let saved_account_id = get_account_id(&saved);
    if current_account_id.is_empty()
        || saved_account_id.is_empty()
        || current_account_id != saved_account_id
    {
        return Ok(());
    }

    let current_bytes = fs::read(&paths.auth_file).map_err(|source| AppError::Io {
        path: paths.auth_file.clone(),
        source,
    })?;
    write_atomic(&target, &current_bytes)
}

fn active_profile(paths: &CswPaths) -> Option<String> {
    load_config_with_paths(paths).ok().and_then(|config| {
        config
            .get("active")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn load_config_with_paths(paths: &CswPaths) -> AppResult<Value> {
    if !paths.config_file.exists() {
        return Ok(json!({}));
    }
    read_json(&paths.config_file)
}

fn save_config_with_paths(paths: &CswPaths, config: &Value) -> AppResult<()> {
    write_json_atomic(&paths.config_file, config)
}

fn update_config_active(paths: &CswPaths, active: &str) -> AppResult<()> {
    let mut config = match load_config_with_paths(paths)? {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    config.insert("active".to_string(), Value::String(active.to_string()));
    save_config_with_paths(paths, &Value::Object(config))
}

fn read_json(path: &Path) -> AppResult<Value> {
    let bytes = fs::read(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| AppError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json_optional(path: &Path) -> AppResult<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }

    match read_json(path) {
        Ok(value) => Ok(Some(value)),
        Err(AppError::Json { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn write_json_atomic(path: &Path, value: &Value) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AppError::Message(format!(
            "Could not serialize JSON for {}: {error}",
            path.display()
        ))
    })?;
    write_atomic(path, &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let tmp = path.with_extension("tmp");
    if let Err(source) = fs::write(&tmp, bytes) {
        return Err(AppError::Io { path: tmp, source });
    }

    if let Err(error) = set_private_permissions(&tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    if let Err(source) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::Io {
            path: path.to_path_buf(),
            source,
        });
    }

    Ok(())
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> AppResult<()> {
    Ok(())
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn ensure_logs_db(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let connection = open_logs_connection(path)?;
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS app_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                level TEXT NOT NULL,
                operation TEXT NOT NULL,
                message TEXT NOT NULL
            )",
            [],
        )
        .map_err(|source| AppError::Message(format!("Could not initialize logs.db: {source}")))?;
    Ok(())
}

fn open_logs_connection(path: &Path) -> AppResult<Connection> {
    Connection::open(path).map_err(|source| {
        AppError::Message(format!(
            "Could not open logs.db at {}: {source}",
            path.display()
        ))
    })
}

fn append_error_log(paths: &CswPaths, operation: &str, error: &AppError) {
    let path = paths.csw_root().join("data/logs.db");
    if ensure_logs_db(&path).is_err() {
        return;
    };
    if let Ok(connection) = open_logs_connection(&path) {
        let _ = connection.execute(
            "INSERT INTO app_logs (timestamp, level, operation, message) VALUES (?1, ?2, ?3, ?4)",
            params![now_seconds(), "error", operation, error.to_string()],
        );
    }
}

/* ─── fetch_available_models ─── */

#[derive(Serialize)]
pub struct ModelEntry {
    pub id: String,
}

#[derive(Serialize)]
pub struct ModelsResult {
    pub models: Vec<ModelEntry>,
    pub error: Option<String>,
}

fn resolve_models_url(base_url: &str, brand: &str, vendor_id: &str) -> String {
    let base = base_url.trim_end_matches('/');

    // Strip Anthropic-specific path suffixes to get the root API domain
    // e.g. https://api.deepseek.com/anthropic → https://api.deepseek.com
    // e.g. https://dashscope.aliyuncs.com/apps/anthropic → https://dashscope.aliyuncs.com
    let root = if brand == "claude" {
        if let Some(idx) = base.find("/anthropic") {
            &base[..idx]
        } else if let Some(idx) = base.find("/apps/anthropic") {
            &base[..idx]
        } else {
            base
        }
    } else {
        base
    };

    // Anthropic native endpoint (no third-party proxy)
    if brand == "claude"
        && !vendor_id.contains("qwen")
        && !vendor_id.contains("deepseek")
        && !vendor_id.contains("zai")
        && !vendor_id.contains("mimo")
        && !root.contains("deepseek")
        && !root.contains("dashscope")
        && !root.contains("z.ai")
        && !root.contains("mimo")
    {
        if root.ends_with("/v1") {
            return format!("{}/models", root);
        }
        return format!("{}/v1/models", root);
    }

    // OpenAI-compatible endpoints (all third-party providers)
    if root.ends_with("/v1") {
        return format!("{}/models", root);
    }
    if root.contains("/compatible-mode/v1") {
        return format!("{}/models", root);
    }
    if root.contains("/api/paas/v4") {
        return format!("{}/models", root);
    }
    // Default: append /v1/models
    format!("{}/v1/models", root)
}

#[tauri::command]
pub async fn fetch_available_models(
    brand: String,
    base_url: String,
    api_key: String,
    vendor_id: String,
) -> ModelsResult {
    if api_key.is_empty() || base_url.is_empty() {
        return ModelsResult {
            models: vec![],
            error: Some("API key and base URL are required".into()),
        };
    }

    let url = resolve_models_url(&base_url, &brand, &vendor_id);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ModelsResult {
                models: vec![],
                error: Some(format!("HTTP client error: {}", e)),
            };
        }
    };

    let mut req = client.get(&url);

    // Anthropic uses x-api-key header; everyone else uses Bearer token
    if brand == "claude"
        && !vendor_id.contains("qwen")
        && !vendor_id.contains("deepseek")
        && !vendor_id.contains("zai")
        && !vendor_id.contains("mimo")
    {
        req = req
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01");
    } else {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return ModelsResult {
                models: vec![],
                error: Some(format!("Request failed: {}", e)),
            };
        }
    };

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return ModelsResult {
            models: vec![],
            error: Some(format!(
                "HTTP {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )),
        };
    }

    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return ModelsResult {
                models: vec![],
                error: Some(format!("Parse error: {}", e)),
            };
        }
    };

    let mut models: Vec<ModelEntry> = Vec::new();

    // OpenAI-compatible format: { data: [{ id: "..." }, ...] }
    if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
        for item in data {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                models.push(ModelEntry { id: id.to_string() });
            }
        }
    }

    models.sort_by(|a, b| a.id.cmp(&b.id));

    ModelsResult {
        models,
        error: None,
    }
}

/* ─── Claude settings.json read/write ─── */

fn claude_settings_path() -> AppResult<PathBuf> {
    let home = dirs::home_dir().ok_or(AppError::HomeDirectoryUnavailable)?;
    Ok(home.join(".claude").join("settings.json"))
}

#[tauri::command]
pub fn read_claude_settings() -> AppResult<String> {
    let path = claude_settings_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| AppError::Message(format!("Failed to read settings.json: {}", e)))?;
        // Validate it's valid JSON
        serde_json::from_str::<Value>(&content)
            .map_err(|e| AppError::Message(format!("Invalid JSON in settings.json: {}", e)))?;
        Ok(content)
    } else {
        Ok("{}".to_string())
    }
}

#[tauri::command]
pub fn write_claude_settings(content: String) -> AppResult<()> {
    // Validate JSON before writing
    serde_json::from_str::<Value>(&content)
        .map_err(|e| AppError::Message(format!("Invalid JSON: {}", e)))?;

    let path = claude_settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::Message(format!("Failed to create directory: {}", e)))?;
    }
    fs::write(&path, &content)
        .map_err(|e| AppError::Message(format!("Failed to write settings.json: {}", e)))?;
    Ok(())
}

/* ─── Codex config.toml read/write ─── */

fn codex_config_path() -> AppResult<PathBuf> {
    let home = dirs::home_dir().ok_or(AppError::HomeDirectoryUnavailable)?;
    Ok(home.join(".codex").join("config.toml"))
}

#[tauri::command]
pub fn read_codex_config() -> AppResult<String> {
    let path = codex_config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| AppError::Message(format!("Failed to read config.toml: {}", e)))?;
        // Parse TOML and convert to JSON for the frontend
        let toml_value: toml::Value = toml::from_str(&content)
            .map_err(|e| AppError::Message(format!("Invalid TOML in config.toml: {}", e)))?;
        let json_str = serde_json::to_string_pretty(&toml_value)
            .map_err(|e| AppError::Message(format!("JSON serialization error: {}", e)))?;
        Ok(json_str)
    } else {
        Ok("{}".to_string())
    }
}

#[tauri::command]
pub fn write_codex_config(json_content: String) -> AppResult<()> {
    // Parse JSON, convert to TOML, write
    let json_value: Value = serde_json::from_str(&json_content)
        .map_err(|e| AppError::Message(format!("Invalid JSON: {}", e)))?;

    // Convert JSON Value to TOML string
    let toml_content = json_to_toml_string(&json_value)?;

    let path = codex_config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::Message(format!("Failed to create directory: {}", e)))?;
    }
    fs::write(&path, &toml_content)
        .map_err(|e| AppError::Message(format!("Failed to write config.toml: {}", e)))?;
    Ok(())
}

fn json_to_toml_string(value: &Value) -> AppResult<String> {
    // Convert serde_json::Value to toml::Value via re-serialization
    let json_str = serde_json::to_string(value)
        .map_err(|e| AppError::Message(format!("JSON serialization error: {}", e)))?;
    let toml_value: toml::Value = toml::from_str(&json_str)
        .map_err(|e| AppError::Message(format!("JSON to TOML conversion error: {}", e)))?;
    toml::to_string_pretty(&toml_value)
        .map_err(|e| AppError::Message(format!("TOML serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use serde_json::{json, Value};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn jwt(payload: Value) -> String {
        let header = json!({"alg": "none", "typ": "JWT"});
        let header = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        format!("{header}.{payload}.")
    }

    fn auth(account_id: &str, email: &str, marker: &str) -> Value {
        json!({
            "tokens": {
                "account_id": account_id,
                "id_token": jwt(json!({
                    "email": email,
                    "exp": 4_102_444_800u64,
                    "https://api.openai.com/auth": {
                        "chatgpt_plan_type": "plus"
                    }
                })),
                "access_token": jwt(json!({
                    "exp": 4_102_444_800u64,
                    "marker": marker
                })),
                "refresh_token": "refresh-token"
            }
        })
    }

    fn expired_auth(account_id: &str, email: &str) -> Value {
        json!({
            "tokens": {
                "account_id": account_id,
                "id_token": jwt(json!({
                    "email": email,
                    "exp": 1u64,
                    "https://api.openai.com/auth": {
                        "chatgpt_plan_type": "free"
                    }
                })),
                "access_token": jwt(json!({
                    "exp": 1u64
                })),
                "refresh_token": "refresh-token"
            }
        })
    }

    fn paths(tmp: &TempDir) -> CswPaths {
        let root = tmp.path();
        CswPaths::new(
            root.join(".codex/auth.json"),
            root.join(".csw/profiles"),
            root.join(".csw/config.json"),
            root.join(".code-switcher"),
        )
    }

    fn write_json(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn account_id(path: &Path) -> String {
        read_json(path)["tokens"]["account_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn marker(path: &Path) -> String {
        let value = read_json(path);
        let token = value["tokens"]["access_token"].as_str().unwrap();
        jwt_payload(token).unwrap()["marker"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn list_profiles_excludes_reserved_files_and_returns_redacted_info() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        let saved_auth = expired_auth("acct-a", "a@example.com");
        let id_token = saved_auth["tokens"]["id_token"].as_str().unwrap();
        let access_token = saved_auth["tokens"]["access_token"].as_str().unwrap();
        write_json(&paths.profile_path("personal"), &saved_auth);
        write_json(
            &paths.profile_path("__current_backup"),
            &auth("acct-b", "b@example.com", "old-b"),
        );
        write_json(&paths.auth_file, &expired_auth("acct-a", "a@example.com"));
        save_config_with_paths(&paths, &json!({"active": "personal"})).unwrap();

        let profiles = list_profiles_with_paths(&paths).unwrap();

        assert_eq!(profiles.len(), 1);
        let profile = &profiles[0];
        assert_eq!(profile.name, "personal");
        assert_eq!(profile.email, "a@example.com");
        assert_eq!(profile.plan, "free");
        assert!(profile.is_active);
        assert!(profile.is_current_auth);
        assert!(profile.id_token_expired);
        assert!(profile.access_token_expired);
        assert_ne!(profile.account_id_hash, "acct-a");
        assert!(!profile.account_id_hash.is_empty());

        let serialized = serde_json::to_string(profile).unwrap();
        assert!(!serialized.contains("acct-a"));
        assert!(!serialized.contains(id_token));
        assert!(!serialized.contains(access_token));
    }

    #[test]
    fn add_profile_refuses_duplicate_accounts_under_a_new_name() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.profile_path("personal"),
            &auth("acct-a", "a@example.com", "old-a"),
        );
        write_json(
            &paths.auth_file,
            &auth("acct-a", "a@example.com", "current-a"),
        );

        let error = add_profile_with_paths(&paths, "work").unwrap_err();

        assert!(error.to_string().contains("already saved"));
        assert!(!paths.profile_path("work").exists());
        assert_eq!(account_id(&paths.profile_path("personal")), "acct-a");
    }

    #[test]
    fn add_existing_name_refuses_a_different_account() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.profile_path("personal"),
            &auth("acct-a", "a@example.com", "old-a"),
        );
        write_json(
            &paths.auth_file,
            &auth("acct-b", "b@example.com", "current-b"),
        );

        let error = add_profile_with_paths(&paths, "personal").unwrap_err();

        assert!(error.to_string().contains("different account"));
        assert_eq!(account_id(&paths.profile_path("personal")), "acct-a");
        assert_eq!(marker(&paths.profile_path("personal")), "old-a");
    }

    #[test]
    fn add_existing_name_refreshes_when_account_matches() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.profile_path("personal"),
            &auth("acct-a", "a@example.com", "old-a"),
        );
        write_json(
            &paths.auth_file,
            &auth("acct-a", "a@example.com", "current-a"),
        );

        let profile = add_profile_with_paths(&paths, "personal").unwrap();

        assert_eq!(profile.name, "personal");
        assert_eq!(account_id(&paths.profile_path("personal")), "acct-a");
        assert_eq!(marker(&paths.profile_path("personal")), "current-a");
    }

    #[test]
    fn switch_does_not_refresh_active_profile_when_current_auth_is_different_account() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.profile_path("personal"),
            &auth("acct-a", "a@example.com", "old-a"),
        );
        write_json(
            &paths.profile_path("work"),
            &auth("acct-b", "b@example.com", "old-b"),
        );
        write_json(
            &paths.auth_file,
            &auth("acct-b", "b@example.com", "current-b"),
        );
        save_config_with_paths(&paths, &json!({"active": "personal"})).unwrap();

        switch_profile_with_paths(&paths, "work").unwrap();

        assert_eq!(account_id(&paths.profile_path("personal")), "acct-a");
        assert_eq!(marker(&paths.profile_path("personal")), "old-a");
        assert_eq!(marker(&paths.auth_file), "old-b");
        assert_eq!(read_json(&paths.config_file)["active"], "work");
    }

    #[test]
    fn rename_updates_active_config() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.profile_path("personal"),
            &auth("acct-a", "a@example.com", "old-a"),
        );
        save_config_with_paths(&paths, &json!({"active": "personal"})).unwrap();

        let profile = rename_profile_with_paths(&paths, "personal", "home").unwrap();

        assert_eq!(profile.name, "home");
        assert!(!paths.profile_path("personal").exists());
        assert!(paths.profile_path("home").exists());
        assert_eq!(read_json(&paths.config_file)["active"], "home");
    }

    #[test]
    fn remove_clears_active_config() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.profile_path("personal"),
            &auth("acct-a", "a@example.com", "old-a"),
        );
        save_config_with_paths(&paths, &json!({"active": "personal"})).unwrap();

        remove_profile_with_paths(&paths, "personal").unwrap();

        assert!(!paths.profile_path("personal").exists());
        assert_eq!(read_json(&paths.config_file)["active"], "");
    }

    #[test]
    fn save_custom_provider_persists_metadata_and_generated_files() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        let providers = save_custom_provider_with_paths(
            &paths,
            CustomProviderInput {
                name: "default".to_string(),
                note: "team account".to_string(),
                website: "https://example.com".to_string(),
                api_key: "sk-test".to_string(),
                api_base: "https://api.example.com/v1".to_string(),
                model: "gpt-5.5".to_string(),
                auth_path: None,
                config_path: None,
                auth_json: None,
                config_toml: None,
            },
        )
        .unwrap();

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "default");
        assert_eq!(providers[0].api_key_preview, "sk-t...test");
        assert!(providers[0].has_api_key);
        assert!(Path::new(&providers[0].auth_path).exists());
        assert!(Path::new(&providers[0].config_path).exists());

        let auth_json = read_json(Path::new(&providers[0].auth_path));
        assert_eq!(auth_json["auth_mode"], "apikey");
        assert_eq!(auth_json["OPENAI_API_KEY"], "sk-test");
        assert_eq!(
            auth_json["custom_provider"]["api_base"],
            "https://api.example.com/v1"
        );

        let config = fs::read_to_string(&providers[0].config_path).unwrap();
        assert!(config.contains("model = \"gpt-5.5\""));
        assert!(config.contains("base_url = \"https://api.example.com/v1\""));
        assert!(config.contains("wire_api = \"responses\""));

        let stored = read_json(&paths.custom_providers_file());
        assert_eq!(stored["providers"][0]["note"], "team account");
        assert_eq!(stored["providers"][0]["apiKey"], "sk-test");
    }

    #[test]
    fn save_custom_provider_rejects_missing_required_fields() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        let error = save_custom_provider_with_paths(
            &paths,
            CustomProviderInput {
                name: "default".to_string(),
                note: String::new(),
                website: String::new(),
                api_key: "sk-test".to_string(),
                api_base: String::new(),
                model: "gpt-5.5".to_string(),
                auth_path: None,
                config_path: None,
                auth_json: None,
                config_toml: None,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("API request URL"));
        assert!(!paths.custom_providers_file().exists());
    }

    #[test]
    fn save_provider_profile_persists_pretty_json_under_brand_config() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        let state = save_provider_profile_with_paths(
            &paths,
            "claude",
            json!({
                "id": "Main Provider",
                "name": "Zulu",
                "apiKey": "secret",
                "nested": {"kept": true}
            }),
        )
        .unwrap();

        let profile_path = tmp.path().join(".csw/config/claude/main-provider.json");
        let saved = fs::read_to_string(&profile_path).unwrap();
        assert!(saved.contains("\n  \"apiKey\": \"secret\""));
        assert_eq!(read_json(&profile_path)["nested"]["kept"], true);
        assert_eq!(state.profiles.len(), 2);
        assert!(state
            .profiles
            .iter()
            .any(|profile| profile["id"] == "Main Provider"));
        assert_eq!(state.active_id, Some("anthropic".to_string()));
    }

    #[test]
    fn save_provider_profile_updates_existing_csw_json_and_list_reads_it_back() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        save_provider_profile_with_paths(
            &paths,
            "codex",
            json!({
                "id": "openai",
                "name": "OpenAI",
                "baseUrl": "https://api.openai.com",
                "modelName": "gpt-5-codex",
                "authJson": "{\"auth_mode\":\"chatgpt\"}"
            }),
        )
        .unwrap();

        let updated = save_provider_profile_with_paths(
            &paths,
            "codex",
            json!({
                "id": "openai",
                "name": "OpenAI Local",
                "baseUrl": "https://gateway.example.test",
                "modelName": "gpt-5.5",
                "authJson": "{\"auth_mode\":\"chatgpt\",\"tokens\":{\"access_token\":\"a\",\"refresh_token\":\"r\"}}"
            }),
        )
        .unwrap();
        let reloaded = list_provider_profiles_with_paths(&paths, "codex").unwrap();
        let profile_path = tmp.path().join(".csw/config/codex/openai.json");

        assert_eq!(updated.profiles.len(), 1);
        assert_eq!(read_json(&profile_path)["name"], "OpenAI Local");
        assert_eq!(
            read_json(&profile_path)["baseUrl"],
            "https://gateway.example.test"
        );
        assert_eq!(reloaded.profiles[0]["name"], "OpenAI Local");
        assert_eq!(reloaded.profiles[0]["modelName"], "gpt-5.5");
    }

    #[test]
    fn activate_provider_profile_toggles_only_selected_profile() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        save_provider_profile_with_paths(&paths, "claude", json!({"id": "a", "name": "Beta"}))
            .unwrap();
        save_provider_profile_with_paths(&paths, "claude", json!({"id": "b", "name": "alpha"}))
            .unwrap();

        let state = activate_provider_profile_with_paths(&paths, "claude", "b").unwrap();

        assert_eq!(state.active_id, Some("b".to_string()));
        assert_eq!(state.profiles[0]["id"], "b");
        assert_eq!(state.profiles[0]["isActive"], true);
        assert_eq!(
            read_json(&paths.provider_profiles_dir("claude").join("anthropic.json"))["isActive"],
            false
        );
        assert_eq!(
            read_json(&paths.provider_profiles_dir("claude").join("a.json"))["isActive"],
            false
        );
        assert_eq!(
            read_json(&paths.provider_profiles_dir("claude").join("b.json"))["isActive"],
            true
        );
    }

    #[test]
    fn activate_codex_provider_writes_runtime_auth_and_config() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        let selected_auth = auth("acct-selected", "selected@example.com", "marker-selected");
        let selected_auth_json = serde_json::to_string_pretty(&selected_auth).unwrap();

        save_provider_profile_with_paths(
            &paths,
            "codex",
            json!({
                "id": "selected",
                "name": "Selected",
                "authJson": selected_auth_json,
                "configToml": "[codex]\nmodel = \"gpt-5.5\"\nmodel_provider = \"openai\""
            }),
        )
        .unwrap();

        activate_provider_profile_with_paths(&paths, "codex", "selected").unwrap();

        assert_eq!(
            get_account_id(&read_json(&paths.auth_file)),
            "acct-selected"
        );
        let config_path = paths.auth_file.parent().unwrap().join("config.toml");
        let config = fs::read_to_string(config_path).unwrap();
        assert!(config.contains("model = \"gpt-5.5\""));
    }

    #[test]
    fn save_and_activate_provider_profile_writes_active_json_for_all_brands() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        for (brand, profile_id, profile) in [
            (
                "claude",
                "anthropic-alt",
                json!({
                    "id": "anthropic-alt",
                    "name": "Anthropic Alt",
                    "mainModel": "claude-sonnet-4-6-20250514"
                }),
            ),
            (
                "codex",
                "openai-alt",
                json!({
                    "id": "openai-alt",
                    "name": "OpenAI Alt",
                    "modelName": "gpt-5.5",
                    "configToml": "[codex]\nmodel = \"gpt-5.5\"\nmodel_provider = \"openai\""
                }),
            ),
            (
                "gemini",
                "google-alt",
                json!({
                    "id": "google-alt",
                    "name": "Google Alt",
                    "model": "gemini-2.5-pro-preview-05-06"
                }),
            ),
        ] {
            let state =
                save_and_activate_provider_profile_with_paths(&paths, brand, profile).unwrap();
            let saved = read_json(
                &paths
                    .provider_profiles_dir(brand)
                    .join(format!("{profile_id}.json")),
            );

            assert_eq!(state.active_id.as_deref(), Some(profile_id));
            assert_eq!(saved["isActive"], true);
        }
    }

    #[test]
    fn save_and_activate_claude_provider_writes_settings_json() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        save_and_activate_provider_profile_with_paths(
            &paths,
            "claude",
            json!({
                "id": "anthropic-alt",
                "name": "Anthropic Alt",
                "apiKey": "sk-ant-test",
                "baseUrl": "https://gateway.example.test",
                "mainModel": "claude-sonnet-4-6-20250514",
                "reasoningModel": "claude-sonnet-4-6-20250514",
                "haikuModel": "claude-haiku-4-5-20251001",
                "sonnetModel": "claude-sonnet-4-6-20250514",
                "opusModel": "claude-opus-4-7-20250416",
                "teammatesMode": true,
                "enableToolSearch": true,
                "hideAiSignature": true,
                "highIntensityThinking": true,
                "disableAutoUpgrade": true,
                "use1MContext": true
            }),
        )
        .unwrap();

        let settings = read_json(&tmp.path().join(".claude/settings.json"));
        let env = settings["env"].as_object().unwrap();

        assert_eq!(env["ANTHROPIC_BASE_URL"], "https://gateway.example.test");
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "sk-ant-test");
        assert_eq!(env["ANTHROPIC_MODEL"], "claude-sonnet-4-6-20250514[1m]");
        assert_eq!(env["HIDE_AI_SIGNATURE"], "true");
        assert_eq!(env["HIGH_INTENSITY_THINKING"], "true");
    }

    #[test]
    fn save_and_activate_provider_profile_logs_activation_errors() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        let error = save_and_activate_provider_profile_with_paths(
            &paths,
            "codex",
            json!({
                "id": "bad-config",
                "name": "Bad Config",
                "configToml": "[codex"
            }),
        )
        .unwrap_err();

        let connection = Connection::open(tmp.path().join(".csw/data/logs.db")).unwrap();
        let (operation, message): (String, String) = connection
            .query_row(
                "SELECT operation, message FROM app_logs ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert!(error.to_string().contains("Invalid TOML"));
        assert_eq!(operation, "save_and_activate_provider_profile");
        assert!(message.contains("Invalid TOML"));
    }

    #[test]
    fn list_provider_profiles_creates_flat_brand_dirs_default_configs_and_empty_logs_db() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);

        let state = list_provider_profiles_with_paths(&paths, "gemini").unwrap();

        assert_eq!(state.profiles.len(), 1);
        assert_eq!(state.profiles[0]["id"], "google");
        assert_eq!(state.profiles[0]["name"], "Google");
        assert_eq!(
            state.profiles[0]["baseUrl"],
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(state.profiles[0]["model"], "gemini-2.5-pro-preview-05-06");
        assert_eq!(state.active_id, Some("google".to_string()));
        for dir in [
            ".csw/config/claude",
            ".csw/config/codex",
            ".csw/config/gemini",
            ".csw/prompts",
            ".csw/diagrams",
            ".csw/data",
            ".csw/backups",
        ] {
            assert!(tmp.path().join(dir).is_dir(), "{dir} should exist");
        }
        assert!(!tmp.path().join(".csw/config/claude/providers").exists());
        assert_eq!(
            read_json(&tmp.path().join(".csw/config/claude/anthropic.json"))["name"],
            "Anthropic"
        );
        assert_eq!(
            read_json(&tmp.path().join(".csw/config/codex/openai.json"))["name"],
            "OpenAI"
        );
        assert_eq!(
            read_json(&tmp.path().join(".csw/config/codex/openai.json"))["modelName"],
            "gpt-5.5"
        );
        assert_eq!(
            read_json(&tmp.path().join(".csw/config/gemini/google.json"))["name"],
            "Google"
        );
        let logs = tmp.path().join(".csw/data/logs.db");
        assert!(logs.is_file());
        let connection = Connection::open(logs).unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM app_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn list_provider_profiles_restores_simplified_default_template_without_api_key() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.provider_profiles_dir("codex").join("openai.json"),
            &json!({
                "id": "openai",
                "name": "OpenAI",
                "apiKey": "",
                "baseUrl": "https://api.openai.com/v1",
                "modelName": "gpt-5.5",
                "authMode": "apikey",
                "isActive": true
            }),
        );

        let state = list_provider_profiles_with_paths(&paths, "codex").unwrap();

        assert_eq!(state.profiles.len(), 1);
        assert_eq!(state.profiles[0]["id"], "openai");
        assert_eq!(state.profiles[0]["authMode"], "chatgpt");
        assert_eq!(state.profiles[0]["modelName"], "gpt-5.5");
        assert_eq!(state.profiles[0]["reasoningEffort"], "xhigh");
        assert!(state.profiles[0]["authJson"]
            .as_str()
            .unwrap()
            .contains("\"tokens\""));
        assert!(state.profiles[0]["configToml"]
            .as_str()
            .unwrap()
            .contains("approvals_reviewer = \"user\""));
    }

    #[test]
    fn list_provider_profiles_restores_old_gpt_5_codex_default_without_auth() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.provider_profiles_dir("codex").join("openai.json"),
            &json!({
                "id": "openai",
                "name": "OpenAI",
                "apiKey": "",
                "modelName": "gpt-5-codex",
                "authMode": "chatgpt",
                "authJson": "{\n  \"auth_mode\": \"chatgpt\",\n  \"tokens\": {\n    \"access_token\": \"\",\n    \"refresh_token\": \"\",\n    \"id_token\": \"\"\n  }\n}",
                "isActive": true
            }),
        );

        let state = list_provider_profiles_with_paths(&paths, "codex").unwrap();

        assert_eq!(state.profiles[0]["modelName"], "gpt-5.5");
        assert!(state.profiles[0]["configToml"]
            .as_str()
            .unwrap()
            .contains("model = \"gpt-5.5\""));
    }

    #[test]
    fn codex_usage_info_parses_account_and_rate_limit_windows() {
        let auth = auth("acct-a", "a@example.com", "marker-a");
        let usage = json!({
            "plan_type": "pro",
            "rate_limit": {
                "primary_window": {
                    "used_percent": 25.4,
                    "reset_after_seconds": 3660
                },
                "secondary_window": {
                    "used_percent": 70,
                    "reset_at": 4102444800u64
                }
            }
        });

        let info = codex_usage_info_from_value(codex_usage_identity(&auth), &usage);

        assert_eq!(info.account_email.as_deref(), Some("a@example.com"));
        assert_eq!(info.account_plan.as_deref(), Some("pro"));
        assert_eq!(
            info.five_hour
                .as_ref()
                .unwrap()
                .remaining_percent
                .unwrap()
                .round(),
            75.0
        );
        assert_eq!(
            info.weekly
                .as_ref()
                .unwrap()
                .remaining_percent
                .unwrap()
                .round(),
            30.0
        );
        assert_eq!(
            info.five_hour.as_ref().unwrap().reset_after_seconds,
            Some(3660)
        );
        assert_eq!(info.weekly.as_ref().unwrap().reset_at, Some(4102444800));
    }

    #[test]
    fn codex_usage_info_uses_usage_response_identity_when_token_identity_is_missing() {
        let auth = json!({
            "tokens": {
                "account_id": "acct-a",
                "access_token": jwt(json!({"exp": 4_102_444_800u64}))
            }
        });
        let usage = json!({
            "email": "response@example.com",
            "account_id": "acct-response",
            "plan_type": "plus",
            "rate_limit": {}
        });

        let info = codex_usage_info_from_value(codex_usage_identity(&auth), &usage);

        assert_eq!(info.account_email.as_deref(), Some("response@example.com"));
        assert_eq!(info.account_plan.as_deref(), Some("plus"));
        assert!(info.account_id_hash.is_some());
    }

    #[test]
    fn codex_usage_auth_is_none_when_provider_auth_is_empty() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.auth_file,
            &auth("acct-local", "local@example.com", "marker-local"),
        );
        let empty_provider_auth = serde_json::to_string_pretty(&json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": "",
                "access_token": "",
                "refresh_token": "",
                "account_id": ""
            }
        }))
        .unwrap();

        let selected = codex_auth_for_usage(&json!({
            "id": "openai",
            "authJson": empty_provider_auth
        }));

        assert!(selected.is_none());
    }

    #[test]
    fn import_codex_auth_reads_fixed_auth_json_into_provider_profile() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.auth_file,
            &auth("acct-a", "a@example.com", "marker-a"),
        );

        let imported = import_codex_auth_with_paths(
            &paths,
            json!({
                "id": "openai",
                "name": "OpenAI",
                "apiKey": "kept-api-key"
            }),
            None,
        )
        .unwrap();

        assert_eq!(imported["id"], "openai");
        assert_eq!(imported["name"], "OpenAI");
        assert_eq!(imported["apiKey"], "kept-api-key");
        assert_eq!(imported["authMode"], "chatgpt");
        let auth_json = imported["authJson"].as_str().unwrap();
        assert!(auth_json.contains("\"account_id\": \"acct-a\""));
        assert!(auth_json.contains("\"access_token\""));
    }

    #[test]
    fn import_codex_auth_can_wait_for_auth_json_to_change_after_login_starts() {
        let tmp = TempDir::new().unwrap();
        let paths = paths(&tmp);
        write_json(
            &paths.auth_file,
            &auth("acct-a", "a@example.com", "marker-a"),
        );

        let error = import_codex_auth_with_paths(
            &paths,
            json!({
                "id": "openai",
                "name": "OpenAI"
            }),
            Some(u64::MAX),
        )
        .unwrap_err();

        assert!(error.to_string().contains("has not changed"));
    }

    #[test]
    fn parse_codex_login_output_extracts_oauth_url_with_ansi_sequences() {
        let output = "Starting local login server on http://localhost:1455.\n\
            If your browser did not open, navigate to this URL to authenticate:\n\
            \u{1b}[94mhttps://auth.openai.com/oauth/authorize?response_type=code&client_id=app_EMoamEEZ73f0CkXaXp7hrann&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback\u{1b}[0m";

        let details = parse_codex_login_output(output).unwrap();

        assert!(details
            .auth_url
            .as_deref()
            .unwrap()
            .starts_with("https://auth.openai.com/oauth/authorize?"));
        assert!(details
            .auth_url
            .as_deref()
            .unwrap()
            .contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
    }

    #[test]
    fn find_codex_binary_uses_codex_bin_override() {
        let tmp = TempDir::new().unwrap();
        let codex = tmp.path().join("custom-codex");
        fs::write(&codex, "").unwrap();

        let found = find_codex_binary_with(Some(codex.clone().into_os_string()), None, None);

        assert_eq!(found.as_deref(), Some(codex.as_path()));
    }

    #[test]
    fn find_codex_binary_checks_path_and_common_homebrew_paths() {
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let codex = bin.join("codex");
        fs::write(&codex, "").unwrap();
        let path_env = env::join_paths([bin]).unwrap();

        let found = find_codex_binary_with(None, Some(path_env), None);

        assert_eq!(found.as_deref(), Some(codex.as_path()));
    }

    #[test]
    fn validate_external_auth_url_only_allows_openai_auth_urls() {
        assert!(validate_external_auth_url("https://auth.openai.com/oauth/authorize?x=1").is_ok());
        assert!(validate_external_auth_url("https://example.com/oauth/authorize").is_err());
        assert!(validate_external_auth_url(" https://auth.openai.com/oauth/authorize").is_err());
        assert!(validate_external_auth_url("https://auth.openai.com/oauth/authorize\n").is_err());
    }
}
