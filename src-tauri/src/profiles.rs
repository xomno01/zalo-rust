use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

const MAX_PROFILE_ID_LEN: usize = 32;
static PROFILES_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub auto_open: bool,
}

/// Validates that a profile ID contains only `[a-z0-9_-]` and has length `1..=32`.
pub fn validate_profile_id(id: &str) -> Result<String, String> {
    if id.is_empty() || id.len() > MAX_PROFILE_ID_LEN {
        return Err(format!(
            "Invalid profile id length (must be 1..={MAX_PROFILE_ID_LEN} chars)"
        ));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err("Invalid profile id: only a-z, 0-9, '-', '_' allowed".to_string());
    }
    Ok(id.to_string())
}

fn fold_vietnamese_char(ch: char) -> char {
    match ch {
        'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ'
        | 'ặ' | 'ẳ' | 'ẵ' => 'a',
        'đ' => 'd',
        'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => 'e',
        'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
        'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ'
        | 'ợ' | 'ở' | 'ỡ' => 'o',
        'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => 'u',
        'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
        _ => ch,
    }
}

/// Produces a safe lowercase ASCII profile ID from an arbitrary display name.
pub fn slugify_profile_id(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;

    for raw in name.trim().chars() {
        for lc in raw.to_lowercase() {
            let folded = fold_vietnamese_char(lc);
            if folded.is_ascii_lowercase() || folded.is_ascii_digit() || folded == '_' {
                out.push(folded);
                prev_dash = false;
            } else if folded == '-' || folded.is_whitespace() {
                if !out.is_empty() && !prev_dash {
                    out.push('-');
                    prev_dash = true;
                }
            }
        }
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.len() > MAX_PROFILE_ID_LEN {
        out.truncate(MAX_PROFILE_ID_LEN);
        while out.ends_with('-') {
            out.pop();
        }
    }

    if out.is_empty() {
        "account".to_string()
    } else {
        out
    }
}

fn deduplicate_profile_id(base_id: &str, existing: &[Profile]) -> String {
    if !existing.iter().any(|p| p.id == base_id) {
        return base_id.to_string();
    }

    let mut suffix_num = 2usize;
    loop {
        let suffix = format!("-{suffix_num}");
        let max_prefix = MAX_PROFILE_ID_LEN.saturating_sub(suffix.len());
        let mut prefix = base_id.chars().take(max_prefix).collect::<String>();
        while prefix.ends_with('-') {
            prefix.pop();
        }
        if prefix.is_empty() {
            prefix = "a".to_string();
        }
        let candidate = format!("{prefix}{suffix}");
        if !existing.iter().any(|p| p.id == candidate) {
            return candidate;
        }
        suffix_num += 1;
    }
}

fn profiles_file_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app_data_dir: {e}"))?;
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app_data_dir: {e}"))?;
    Ok(app_dir.join("profiles.json"))
}

fn save_profiles_to_path(path: &Path, profiles: &[Profile]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

fn load_or_seed_profiles_from_path(path: &Path) -> Result<Vec<Profile>, String> {
    if !path.exists() {
        let seeded = vec![Profile {
            id: "default".to_string(),
            name: "Tài khoản chính".to_string(),
            auto_open: false,
        }];
        save_profiles_to_path(path, &seeded)?;
        return Ok(seeded);
    }

    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str::<Vec<Profile>>(&raw)
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

pub fn list_profiles(app: &tauri::AppHandle) -> Result<Vec<Profile>, String> {
    let _guard = PROFILES_MUTEX
        .lock()
        .map_err(|_| "Profiles lock poisoned".to_string())?;
    let path = profiles_file_path(app)?;
    load_or_seed_profiles_from_path(&path)
}

pub fn create_profile(
    app: &tauri::AppHandle,
    name: &str,
    auto_open: bool,
) -> Result<Profile, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Tên tài khoản không được để trống".to_string());
    }

    let _guard = PROFILES_MUTEX
        .lock()
        .map_err(|_| "Profiles lock poisoned".to_string())?;
    let path = profiles_file_path(app)?;
    let mut profiles = load_or_seed_profiles_from_path(&path)?;

    let base_id = slugify_profile_id(trimmed);
    let unique_id = deduplicate_profile_id(&base_id, &profiles);
    validate_profile_id(&unique_id)?;

    let profile = Profile {
        id: unique_id,
        name: trimmed.to_string(),
        auto_open,
    };
    profiles.push(profile.clone());
    save_profiles_to_path(&path, &profiles)?;
    Ok(profile)
}

pub fn toggle_auto_open(
    app: &tauri::AppHandle,
    id: &str,
    auto_open: bool,
) -> Result<Profile, String> {
    let valid_id = validate_profile_id(id)?;
    let _guard = PROFILES_MUTEX
        .lock()
        .map_err(|_| "Profiles lock poisoned".to_string())?;
    let path = profiles_file_path(app)?;
    let mut profiles = load_or_seed_profiles_from_path(&path)?;

    let target = profiles
        .iter_mut()
        .find(|p| p.id == valid_id)
        .ok_or_else(|| format!("Không tìm thấy tài khoản '{valid_id}'"))?;
    target.auto_open = auto_open;
    let updated = target.clone();
    save_profiles_to_path(&path, &profiles)?;
    Ok(updated)
}

pub fn get_profile(app: &tauri::AppHandle, id: &str) -> Result<Profile, String> {
    let valid_id = validate_profile_id(id)?;
    let _guard = PROFILES_MUTEX
        .lock()
        .map_err(|_| "Profiles lock poisoned".to_string())?;
    let path = profiles_file_path(app)?;
    let profiles = load_or_seed_profiles_from_path(&path)?;

    profiles
        .into_iter()
        .find(|p| p.id == valid_id)
        .ok_or_else(|| format!("Không tìm thấy tài khoản '{valid_id}'"))
}

pub fn profile_data_dir(app: &tauri::AppHandle, id: &str) -> Result<PathBuf, String> {
    let valid_id = validate_profile_id(id)?;
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app_data_dir: {e}"))?;
    let dir = app_dir.join("profiles").join(valid_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create profile data dir: {e}"))?;
    Ok(dir)
}

/// Generates a deterministic 16-byte UUID v5-formatted identifier for macOS WKWebView `data_store_identifier`.
pub fn profile_store_uuid(id: &str) -> [u8; 16] {
    const FNV_OFFSET_1: u64 = 0xcbf29ce484222325;
    const FNV_OFFSET_2: u64 = 0xaf63bd4c8601b7df;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut h1 = FNV_OFFSET_1;
    let mut h2 = FNV_OFFSET_2;

    for &b in b"zalo-rust:".iter().chain(id.as_bytes().iter()) {
        h1 ^= u64::from(b);
        h1 = h1.wrapping_mul(FNV_PRIME);

        h2 = h2.wrapping_mul(FNV_PRIME);
        h2 ^= u64::from(b).wrapping_add(0x9e);
    }

    let mut uuid = [0u8; 16];
    uuid[..8].copy_from_slice(&h1.to_be_bytes());
    uuid[8..].copy_from_slice(&h2.to_be_bytes());

    // Set RFC 4122 version 5 and variant 1 bits
    uuid[6] = (uuid[6] & 0x0f) | 0x50;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    uuid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_profile_id_accepts_valid_and_rejects_unsafe() {
        assert_eq!(validate_profile_id("default").unwrap(), "default");
        assert_eq!(validate_profile_id("work_acc-01").unwrap(), "work_acc-01");

        assert!(validate_profile_id("").is_err());
        assert!(validate_profile_id("UPPER").is_err());
        assert!(validate_profile_id("../etc").is_err());
        assert!(validate_profile_id("a/b").is_err());
        assert!(validate_profile_id("a\\b").is_err());
        assert!(validate_profile_id("has space").is_err());
        assert!(validate_profile_id(&"a".repeat(33)).is_err());
        assert!(validate_profile_id(&"a".repeat(32)).is_ok());
    }

    #[test]
    fn slugify_profile_id_handles_vietnamese_symbols_and_limits() {
        assert_eq!(slugify_profile_id("Tài khoản chính"), "tai-khoan-chinh");
        assert_eq!(slugify_profile_id("Công việc - Đồng đội"), "cong-viec-dong-doi");
        assert_eq!(slugify_profile_id("Shop_01!@#"), "shop_01");
        assert_eq!(slugify_profile_id("   ---   "), "account");
        assert_eq!(slugify_profile_id(""), "account");

        let long_slug = slugify_profile_id(&"tài khoản rất dài ".repeat(5));
        assert!(long_slug.len() <= MAX_PROFILE_ID_LEN);
        assert!(!long_slug.ends_with('-'));
        assert!(validate_profile_id(&long_slug).is_ok());
    }

    #[test]
    fn deduplicate_profile_id_appends_numeric_suffix_within_limit() {
        let existing = vec![
            Profile {
                id: "work".to_string(),
                name: "Work".to_string(),
                auto_open: false,
            },
            Profile {
                id: "work-2".to_string(),
                name: "Work 2".to_string(),
                auto_open: false,
            },
        ];
        assert_eq!(deduplicate_profile_id("work", &existing), "work-3");

        let max_base = "a".repeat(32);
        let max_existing = vec![Profile {
            id: max_base.clone(),
            name: "Max".to_string(),
            auto_open: false,
        }];
        let deduped = deduplicate_profile_id(&max_base, &max_existing);
        assert_eq!(deduped.len(), 32);
        assert!(deduped.ends_with("-2"));
        assert!(validate_profile_id(&deduped).is_ok());
    }

    #[test]
    fn profile_store_uuid_is_deterministic_distinct_and_rfc4122() {
        let u1 = profile_store_uuid("default");
        let u1_again = profile_store_uuid("default");
        let u2 = profile_store_uuid("work");

        assert_eq!(u1, u1_again);
        assert_ne!(u1, u2);
        assert_eq!(u1[6] & 0xf0, 0x50);
        assert_eq!(u1[8] & 0xc0, 0x80);
    }
}
