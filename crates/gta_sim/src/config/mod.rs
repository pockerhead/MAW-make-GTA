use bevy::prelude::Resource;
use serde::de::DeserializeOwned;
use std::{fmt, fs, path::PathBuf};

#[derive(Resource, Clone, Debug)]
pub struct ConfigRoot(pub PathBuf);

impl ConfigRoot {
    pub fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

#[derive(Debug)]
pub struct ConfigError {
    pub path: PathBuf,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ConfigError {}

pub fn load_config<T: DeserializeOwned>(root: &ConfigRoot, rel: &str) -> Result<T, ConfigError> {
    let path = root.path(rel);
    let contents = fs::read_to_string(&path).map_err(|err| ConfigError {
        path: path.clone(),
        message: err.to_string(),
    })?;
    ron::from_str(&contents).map_err(|err| ConfigError {
        path,
        message: err.to_string(),
    })
}
