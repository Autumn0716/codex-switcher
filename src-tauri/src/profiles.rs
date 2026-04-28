use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const VALID_NAME_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";

type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug)]
pub struct CswPaths {
    auth_file: PathBuf,
    profiles_dir: PathBuf,
    config_file: PathBuf,
}

impl CswPaths {
    fn production() -> AppResult<Self> {
        let home = dirs::home_dir().ok_or(AppError::HomeDirectoryUnavailable)?;
        Ok(Self::new(
            home.join(".codex/auth.json"),
            home.join(".csw/profiles"),
            home.join(".csw/config.json"),
        ))
    }

    fn new(auth_file: PathBuf, profiles_dir: PathBuf, config_file: PathBuf) -> Self {
        Self {
            auth_file,
            profiles_dir,
            config_file,
        }
    }

    fn profile_path(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(format!("{name}.json"))
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
                }))
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
                }))
            }
        })
    }

    fn paths(tmp: &TempDir) -> CswPaths {
        let root = tmp.path();
        CswPaths::new(
            root.join(".codex/auth.json"),
            root.join(".csw/profiles"),
            root.join(".csw/config.json"),
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
}
