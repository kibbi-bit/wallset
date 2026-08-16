use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    #[default]
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideshowOrder {
    DateAdded,
    #[default]
    Random,
}

impl FitMode {
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Fit,
            2 => Self::Stretch,
            3 => Self::Center,
            4 => Self::Tile,
            _ => Self::Fill,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Self::Fill => 0,
            Self::Fit => 1,
            Self::Stretch => 2,
            Self::Center => 3,
            Self::Tile => 4,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WallpaperMode {
    Picture {
        path: PathBuf,
    },
    Color {
        rgb: [u8; 3],
    },
    Slideshow {
        folder: PathBuf,
        interval_seconds: u64,
        #[serde(default)]
        order: SlideshowOrder,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MonitorConfig {
    pub mode: WallpaperMode,
    #[serde(default)]
    pub current_image: Option<PathBuf>,
    #[serde(default)]
    pub remaining_shuffle_deck: Vec<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub shared_fit: FitMode,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default)]
    pub monitors: BTreeMap<String, MonitorConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            shared_fit: FitMode::Fill,
            launch_at_login: false,
            monitors: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Paths {
    pub config_file: PathBuf,
    pub color_cache: PathBuf,
}

impl Paths {
    pub fn discover() -> io::Result<Self> {
        let dirs = ProjectDirs::from("dev", "Wallset", "Wallset")
            .ok_or_else(|| io::Error::other("Windows application-data directory is unavailable"))?;
        let config_dir = dirs.config_dir();
        let color_cache = dirs.cache_dir().join("colors");
        fs::create_dir_all(config_dir)?;
        fs::create_dir_all(&color_cache)?;
        Ok(Self {
            config_file: config_dir.join("config.json"),
            color_cache,
        })
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> io::Result<Self> {
        match fs::read(path) {
            Ok(bytes) => {
                let config: Self = serde_json::from_slice(&bytes)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                if config.schema_version != SCHEMA_VERSION {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unsupported configuration schema {}", config.schema_version),
                    ));
                }
                Ok(config)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, bytes)?;
        replace_file(&temp, path)
    }
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::Storage::FileSystem::{
            MOVE_FILE_FLAGS, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVE_FILE_FLAGS(MOVEFILE_REPLACE_EXISTING.0 | MOVEFILE_WRITE_THROUGH.0),
        )
        .map_err(|error| io::Error::other(error.to_string()))
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let mut config = AppConfig::default();
        config.monitors.insert(
            "monitor-a".into(),
            MonitorConfig {
                mode: WallpaperMode::Color { rgb: [1, 2, 3] },
                current_image: None,
                remaining_shuffle_deck: Vec::new(),
            },
        );
        let encoded = serde_json::to_vec(&config).unwrap();
        let decoded: AppConfig = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.schema_version, SCHEMA_VERSION);
        assert_eq!(decoded.monitors["monitor-a"], config.monitors["monitor-a"]);
    }

    #[test]
    fn legacy_device_path_is_ignored() {
        let json = r#"{"device_path":"monitor-a","mode":{"kind":"color","rgb":[1,2,3]}}"#;
        let config: MonitorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.mode, WallpaperMode::Color { rgb: [1, 2, 3] });
    }

    #[test]
    fn fit_indices_are_stable() {
        for index in 0..=4 {
            assert_eq!(FitMode::from_index(index).index(), index);
        }
    }

    #[test]
    fn legacy_slideshow_defaults_to_random_order() {
        let json = r#"{"kind":"slideshow","folder":"images","interval_seconds":900}"#;
        let mode: WallpaperMode = serde_json::from_str(json).unwrap();
        assert_eq!(
            mode,
            WallpaperMode::Slideshow {
                folder: "images".into(),
                interval_seconds: 900,
                order: SlideshowOrder::Random,
            }
        );
    }
}
