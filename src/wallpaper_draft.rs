use std::path::PathBuf;

use crate::config::WallpaperMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntervalUnit {
    Minutes,
    Hours,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WallpaperDraft {
    Current {
        path: Option<PathBuf>,
    },
    Picture {
        path: PathBuf,
    },
    Color {
        value: String,
    },
    Slideshow {
        folder: PathBuf,
        interval_value: String,
        interval_unit: IntervalUnit,
    },
}

impl WallpaperDraft {
    pub fn from_mode(mode: &WallpaperMode) -> Self {
        match mode {
            WallpaperMode::Picture { path } => Self::Picture { path: path.clone() },
            WallpaperMode::Color { rgb } => Self::Color {
                value: format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
            },
            WallpaperMode::Slideshow {
                folder,
                interval_seconds,
            } => {
                let (interval_value, interval_unit) = if interval_seconds % 3600 == 0 {
                    ((interval_seconds / 3600).to_string(), IntervalUnit::Hours)
                } else {
                    ((interval_seconds / 60).to_string(), IntervalUnit::Minutes)
                };
                Self::Slideshow {
                    folder: folder.clone(),
                    interval_value,
                    interval_unit,
                }
            }
        }
    }

    pub fn apply_intent(&self) -> Result<WallpaperMode, String> {
        match self {
            Self::Current { .. } => Err("Choose Picture, Color, or Slideshow".into()),
            Self::Picture { path } => Ok(WallpaperMode::Picture { path: path.clone() }),
            Self::Color { value } => parse_color(value)
                .map(|rgb| WallpaperMode::Color { rgb })
                .ok_or_else(|| "Enter a color as #RRGGBB".into()),
            Self::Slideshow {
                folder,
                interval_value,
                interval_unit,
            } => {
                let value: u64 = interval_value
                    .trim()
                    .parse()
                    .map_err(|_| "Enter a whole-number interval".to_string())?;
                let multiplier = match interval_unit {
                    IntervalUnit::Minutes => 60,
                    IntervalUnit::Hours => 3600,
                };
                let interval_seconds = value
                    .checked_mul(multiplier)
                    .ok_or_else(|| "The interval is too large".to_string())?;
                if interval_seconds < 60 {
                    return Err("The minimum interval is one minute".into());
                }
                Ok(WallpaperMode::Slideshow {
                    folder: folder.clone(),
                    interval_seconds,
                })
            }
        }
    }

    pub fn color_rgb(&self) -> Option<[u8; 3]> {
        if let Self::Color { value } = self {
            parse_color(value)
        } else {
            None
        }
    }
}

fn parse_color(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wallpaper_mode_round_trips_through_a_draft() {
        let modes = [
            WallpaperMode::Picture {
                path: "a.jpg".into(),
            },
            WallpaperMode::Color {
                rgb: [0x12, 0xab, 0xef],
            },
            WallpaperMode::Slideshow {
                folder: "images".into(),
                interval_seconds: 3600,
            },
            WallpaperMode::Slideshow {
                folder: "images".into(),
                interval_seconds: 900,
            },
        ];
        for mode in modes {
            assert_eq!(
                WallpaperDraft::from_mode(&mode).apply_intent().unwrap(),
                mode
            );
        }
    }

    #[test]
    fn interval_normalizes_to_hours_only_when_exact() {
        let exact = WallpaperDraft::from_mode(&WallpaperMode::Slideshow {
            folder: "images".into(),
            interval_seconds: 7200,
        });
        let partial = WallpaperDraft::from_mode(&WallpaperMode::Slideshow {
            folder: "images".into(),
            interval_seconds: 5400,
        });
        assert!(matches!(
            exact,
            WallpaperDraft::Slideshow {
                interval_unit: IntervalUnit::Hours,
                ..
            }
        ));
        assert!(matches!(
            partial,
            WallpaperDraft::Slideshow {
                interval_unit: IntervalUnit::Minutes,
                ..
            }
        ));
    }

    #[test]
    fn invalid_values_do_not_create_an_apply_intent() {
        assert!(
            WallpaperDraft::Color {
                value: "bad".into()
            }
            .apply_intent()
            .is_err()
        );
        assert!(
            WallpaperDraft::Slideshow {
                folder: "images".into(),
                interval_value: "0".into(),
                interval_unit: IntervalUnit::Minutes
            }
            .apply_intent()
            .is_err()
        );
    }
}
