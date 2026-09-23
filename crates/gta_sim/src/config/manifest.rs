use super::ConfigRoot;
use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf};

/// Path of the third-party asset manifest, relative to the assets root.
pub const THIRD_PARTY_MANIFEST: &str = "third_party/manifest.ron";
/// Directory of fetched packs, relative to the assets root.
pub const THIRD_PARTY_DIR: &str = "third_party";

/// Third-party asset packs; `tools/fetch_assets.py` checks the same rules as `validate`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ThirdPartyManifest {
    pub packs: Vec<AssetPack>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AssetPack {
    pub name: String,
    pub version: String,
    pub page: String,
    pub url: String,
    pub archive_sha256: String,
    pub license: AssetLicense,
    pub license_file: String,
    pub files: Vec<PackFile>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetLicense {
    CC0,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PackFile {
    pub archive: String,
    pub path: String,
    pub sha256: String,
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_safe_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && !value.contains(':')
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

impl AssetPack {
    fn validate(&self) -> Result<(), String> {
        let name = &self.name;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(format!("pack {name:?}: name must match ^[a-z0-9-]+$"));
        }
        if self.version.is_empty() {
            return Err(format!("pack {name}: version is empty"));
        }
        let page = format!("https://kenney.nl/assets/{name}");
        if self.page != page {
            return Err(format!(
                "pack {name}: page {:?}, expected {page:?}",
                self.page
            ));
        }
        let prefix = format!("https://kenney.nl/media/pages/assets/{name}/");
        if !self.url.starts_with(&prefix) || !self.url.ends_with(".zip") {
            return Err(format!(
                "pack {name}: url {:?} must start with {prefix:?} and end with .zip",
                self.url
            ));
        }
        if !is_sha256(&self.archive_sha256) {
            return Err(format!(
                "pack {name}: archive_sha256 {:?} is not 64 lowercase hex digits",
                self.archive_sha256
            ));
        }
        if self.files.is_empty() {
            return Err(format!("pack {name}: files is empty"));
        }
        let (mut paths, mut archives) = (HashSet::new(), HashSet::new());
        for file in &self.files {
            for (field, value) in [("path", &file.path), ("archive", &file.archive)] {
                if !is_safe_path(value) {
                    return Err(format!("pack {name}: unsafe {field} {value:?}"));
                }
            }
            if !is_sha256(&file.sha256) {
                return Err(format!(
                    "pack {name}: sha256 {:?} of {} is not 64 lowercase hex digits",
                    file.sha256, file.path
                ));
            }
            if !paths.insert(file.path.as_str()) {
                return Err(format!("pack {name}: duplicate path {:?}", file.path));
            }
            if !archives.insert(file.archive.as_str()) {
                return Err(format!("pack {name}: duplicate archive {:?}", file.archive));
            }
        }
        if !paths.contains(self.license_file.as_str()) {
            return Err(format!(
                "pack {name}: license_file {:?} is not listed in files",
                self.license_file
            ));
        }
        Ok(())
    }
}

impl ThirdPartyManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.packs.is_empty() {
            return Err("packs is empty".into());
        }
        let mut names = HashSet::new();
        for pack in &self.packs {
            if !names.insert(pack.name.as_str()) {
                return Err(format!("duplicate pack name {:?}", pack.name));
            }
            pack.validate()?;
        }
        Ok(())
    }

    /// Files of the manifest that are absent under `<root>/third_party/<pack>/`.
    pub fn missing_files(&self, root: &ConfigRoot) -> Vec<PathBuf> {
        self.packs
            .iter()
            .flat_map(|pack| {
                pack.files
                    .iter()
                    .map(move |file| root.path(THIRD_PARTY_DIR).join(&pack.name).join(&file.path))
            })
            .filter(|path| !path.is_file())
            .collect()
    }

    /// True when `asset_path` is `third_party/<pack>/<path>` of a listed file.
    pub fn contains_asset(&self, asset_path: &str) -> bool {
        self.packs.iter().any(|pack| {
            pack.files
                .iter()
                .any(|file| asset_path == format!("{THIRD_PARTY_DIR}/{}/{}", pack.name, file.path))
        })
    }
}
