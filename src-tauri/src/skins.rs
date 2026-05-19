use std::{
    collections::HashSet,
    fs::{self, create_dir_all},
    path::{Path, PathBuf},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::Settings;

const DEFAULT_SKIN_ID: &str = "base-2.91";
const DEFAULT_SKIN_NAME: &str = "Winamp 2.91";
const PROJECT_SKINS_DIR: &str = "skins";

const BUNDLED_SKIN_ASSETS: &[&str] = &[
    "BALANCE.BMP",
    "CBUTTONS.BMP",
    "CLOSE.CUR",
    "EQSLID.CUR",
    "MAIN.BMP",
    "MAINMENU.CUR",
    "MONOSTER.BMP",
    "NUMBERS.BMP",
    "PLAYPAUS.BMP",
    "PLEDIT.BMP",
    "POSBAR.BMP",
    "SHUFREP.BMP",
    "TEXT.BMP",
    "TITLEBAR.BMP",
    "TITLEBAR.CUR",
    "VOLBAL.CUR",
    "VOLUME.BMP",
];

#[derive(Debug, Clone, Serialize)]
pub struct SkinAsset {
    name: String,
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkinInfo {
    id: String,
    name: String,
    bundled: bool,
    assets: Vec<SkinAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkinLibrary {
    current_skin_id: String,
    skins_dir: String,
    skins: Vec<SkinInfo>,
}

fn get_skins_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let path = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not find the app data directory ({e:?})"))?
        .join("skins");
    create_dir_all(&path).map_err(|e| format!("Could not create skins directory ({e:?})"))?;
    Ok(path)
}

fn get_source_project_skins_dir() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|project_root| project_root.join(PROJECT_SKINS_DIR))
}

fn get_resource_project_skins_dir(app_handle: &AppHandle) -> Option<PathBuf> {
    app_handle
        .path()
        .resource_dir()
        .ok()
        .map(|resource_dir| resource_dir.join(PROJECT_SKINS_DIR))
}

fn bundled_skin() -> SkinInfo {
    SkinInfo {
        id: DEFAULT_SKIN_ID.to_string(),
        name: DEFAULT_SKIN_NAME.to_string(),
        bundled: true,
        assets: BUNDLED_SKIN_ASSETS
            .iter()
            .map(|name| SkinAsset {
                name: name.to_string(),
                path: None,
            })
            .collect(),
    }
}

fn display_name_from_id(id: &str) -> String {
    id.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for character in name.chars().flat_map(|c| c.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "skin".to_string()
    } else {
        slug
    }
}

fn uppercase_file_name(path: &Path) -> Option<String> {
    path.file_name()
        .map(|file_name| file_name.to_string_lossy().to_ascii_uppercase())
}

fn skin_root(path: &Path) -> Option<PathBuf> {
    if contains_asset(path, "MAIN.BMP") {
        return Some(path.to_path_buf());
    }

    fs::read_dir(path)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_dir() && contains_asset(path, "MAIN.BMP"))
}

fn contains_asset(path: &Path, asset: &str) -> bool {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .any(|entry| {
            entry.path().is_file() && uppercase_file_name(&entry.path()).as_deref() == Some(asset)
        })
}

fn read_imported_assets(path: &Path) -> Vec<SkinAsset> {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }

            Some(SkinAsset {
                name: uppercase_file_name(&path)?,
                path: Some(path.to_string_lossy().to_string()),
            })
        })
        .collect()
}

fn read_skin_info(path: &Path) -> Option<SkinInfo> {
    let root = skin_root(path)?;
    let id = root.file_name()?.to_string_lossy().to_string();

    Some(SkinInfo {
        name: display_name_from_id(&id),
        id,
        bundled: false,
        assets: read_imported_assets(&root),
    })
}

fn list_skins_in_dir(skins_dir: &Path) -> Vec<SkinInfo> {
    let mut skins: Vec<SkinInfo> = fs::read_dir(skins_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| entry.path().is_dir().then(|| entry.path()))
        .filter_map(|path| read_skin_info(&path))
        .collect();

    skins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    skins
}

fn list_project_skins(app_handle: &AppHandle) -> Vec<SkinInfo> {
    get_source_project_skins_dir()
        .into_iter()
        .chain(get_resource_project_skins_dir(app_handle))
        .flat_map(|skins_dir| list_skins_in_dir(&skins_dir))
        .collect()
}

fn build_skin_library(app_handle: &AppHandle) -> Result<SkinLibrary, String> {
    let skins_dir = get_skins_dir(app_handle)?;
    let mut skins = vec![bundled_skin()];
    let mut skin_ids: HashSet<String> = skins.iter().map(|skin| skin.id.clone()).collect();

    for skin in list_skins_in_dir(&skins_dir)
        .into_iter()
        .chain(list_project_skins(app_handle))
    {
        if skin_ids.insert(skin.id.clone()) {
            skins.push(skin);
        }
    }

    let available_ids: HashSet<&str> = skins.iter().map(|skin| skin.id.as_str()).collect();
    let current_skin_id = Settings::current().skin.current_skin_id.clone();
    let current_skin_id = if available_ids.contains(current_skin_id.as_str()) {
        current_skin_id
    } else {
        DEFAULT_SKIN_ID.to_string()
    };

    Ok(SkinLibrary {
        current_skin_id,
        skins_dir: skins_dir.to_string_lossy().to_string(),
        skins,
    })
}

#[tauri::command]
pub fn get_skin_library(app_handle: AppHandle) -> Result<SkinLibrary, String> {
    build_skin_library(&app_handle)
}

#[tauri::command]
pub fn set_current_skin(id: String, app_handle: AppHandle) -> Result<SkinLibrary, String> {
    let library = build_skin_library(&app_handle)?;
    if !library.skins.iter().any(|skin| skin.id == id) {
        return Err(format!("Skin '{id}' is not installed"));
    }

    Settings::current_mut().skin.current_skin_id = id;
    let library = build_skin_library(&app_handle)?;
    app_handle
        .emit("skinChanged", &library)
        .map_err(|e| format!("Could not broadcast skin change ({e:?})"))?;
    Ok(library)
}

fn unique_destination(base_dir: &Path, base_id: &str) -> PathBuf {
    let mut id = base_id.to_string();
    let mut counter = 2;
    while base_dir.join(&id).exists() {
        id = format!("{base_id}-{counter}");
        counter += 1;
    }
    base_dir.join(id)
}

#[tauri::command]
pub fn import_skin_folder(
    source_dir: String,
    app_handle: AppHandle,
) -> Result<SkinLibrary, String> {
    let source_dir = PathBuf::from(source_dir);
    if !source_dir.is_dir() {
        return Err("Please choose an extracted Winamp skin folder".to_string());
    }

    let source_root = skin_root(&source_dir)
        .ok_or("That folder does not look like a Winamp skin: MAIN.BMP was not found")?;
    let skins_dir = get_skins_dir(&app_handle)?;
    let base_name = source_root
        .file_name()
        .map(|file_name| file_name.to_string_lossy().to_string())
        .unwrap_or_else(|| "skin".to_string());
    let destination = unique_destination(&skins_dir, &slugify(&base_name));
    create_dir_all(&destination)
        .map_err(|e| format!("Could not create imported skin folder ({e:?})"))?;

    let mut copied_files = 0;
    for entry in fs::read_dir(&source_root)
        .map_err(|e| format!("Could not read selected skin folder ({e:?})"))?
    {
        let entry = entry.map_err(|e| format!("Could not read skin file ({e:?})"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = uppercase_file_name(&path) else {
            continue;
        };
        fs::copy(&path, destination.join(file_name))
            .map_err(|e| format!("Could not copy skin file '{path:?}' ({e:?})"))?;
        copied_files += 1;
    }

    if copied_files == 0 || !contains_asset(&destination, "MAIN.BMP") {
        let _ = fs::remove_dir_all(&destination);
        return Err("That folder does not include the core Winamp skin bitmaps".to_string());
    }

    let imported_id = destination
        .file_name()
        .expect("a destination folder name")
        .to_string_lossy()
        .to_string();
    Settings::current_mut().skin.current_skin_id = imported_id;
    let library = build_skin_library(&app_handle)?;
    app_handle
        .emit("skinChanged", &library)
        .map_err(|e| format!("Could not broadcast skin import ({e:?})"))?;
    Ok(library)
}
