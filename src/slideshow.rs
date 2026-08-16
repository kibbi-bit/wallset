use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use rand::seq::SliceRandom;

#[derive(Clone, Debug, PartialEq)]
pub struct Slideshow {
    pub folder: PathBuf,
    pub interval_seconds: u64,
    pub current_image: Option<PathBuf>,
    pub remaining_deck: Vec<PathBuf>,
}

pub struct Lifecycle<C = FileSystemCatalog> {
    catalog: C,
    deadlines: BTreeMap<String, Instant>,
}

impl Default for Lifecycle<FileSystemCatalog> {
    fn default() -> Self {
        Self::new(FileSystemCatalog)
    }
}

impl<C: ImageCatalog> Lifecycle<C> {
    pub fn new(catalog: C) -> Self {
        Self {
            catalog,
            deadlines: BTreeMap::new(),
        }
    }

    pub fn start(
        &mut self,
        monitor_id: &str,
        folder: PathBuf,
        interval_seconds: u64,
        now: Instant,
        mut apply: impl FnMut(&Path) -> Result<(), String>,
    ) -> Result<Slideshow, String> {
        if interval_seconds < 60 {
            return Err("The minimum slideshow interval is one minute".into());
        }
        let mut slideshow = Slideshow {
            folder,
            interval_seconds,
            current_image: None,
            remaining_deck: Vec::new(),
        };
        self.advance_image(&mut slideshow, &mut apply)?;
        self.schedule(monitor_id, now, interval_seconds);
        Ok(slideshow)
    }

    pub fn restore(
        &mut self,
        monitor_id: &str,
        slideshow: &mut Slideshow,
        now: Instant,
        mut apply: impl FnMut(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        self.advance_image(slideshow, &mut apply)?;
        self.schedule(monitor_id, now, slideshow.interval_seconds);
        Ok(())
    }

    pub fn advance(
        &mut self,
        monitor_id: &str,
        slideshow: &mut Slideshow,
        now: Instant,
        mut apply: impl FnMut(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let result = self.advance_image(slideshow, &mut apply);
        match result {
            Ok(()) => self.schedule(monitor_id, now, slideshow.interval_seconds),
            Err(_) => self.cancel(monitor_id),
        }
        result
    }

    pub fn cancel(&mut self, monitor_id: &str) {
        self.deadlines.remove(monitor_id);
    }

    pub fn due(&mut self, now: Instant) -> Vec<String> {
        let due: Vec<_> = self
            .deadlines
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &due {
            self.deadlines.remove(id);
        }
        due
    }

    fn schedule(&mut self, monitor_id: &str, now: Instant, interval_seconds: u64) {
        self.deadlines.insert(
            monitor_id.to_owned(),
            now + Duration::from_secs(interval_seconds),
        );
    }

    fn advance_image(
        &self,
        slideshow: &mut Slideshow,
        apply: &mut impl FnMut(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let images = self.catalog.images(&slideshow.folder)?;
        if images.is_empty() {
            return Err("The slideshow folder is empty".into());
        }
        for _ in 0..images.len() {
            let Some(next) = next_image(
                &images,
                slideshow.current_image.as_deref(),
                &mut slideshow.remaining_deck,
            ) else {
                break;
            };
            if apply(&next).is_ok() {
                slideshow.current_image = Some(next);
                return Ok(());
            }
        }
        Err("No slideshow image could be applied".into())
    }
}

pub trait ImageCatalog {
    fn images(&self, folder: &Path) -> Result<Vec<PathBuf>, String>;
}

pub struct FileSystemCatalog;

impl ImageCatalog for FileSystemCatalog {
    fn images(&self, folder: &Path) -> Result<Vec<PathBuf>, String> {
        if !folder.is_dir() {
            return Err("Choose an existing slideshow folder".into());
        }
        let entries =
            fs::read_dir(folder).map_err(|error| format!("Cannot read folder: {error}"))?;
        let mut images = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_supported_image(&path) {
                images.push(path);
            }
        }
        images.sort_by_key(|path| path.to_string_lossy().to_lowercase());
        Ok(images)
    }
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp"
            )
        })
}

fn next_image(
    images: &[PathBuf],
    current: Option<&Path>,
    remaining: &mut Vec<PathBuf>,
) -> Option<PathBuf> {
    let available: HashSet<&Path> = images.iter().map(PathBuf::as_path).collect();
    remaining.retain(|path| available.contains(path.as_path()));
    if remaining.is_empty() {
        *remaining = images
            .iter()
            .filter(|path| Some(path.as_path()) != current)
            .cloned()
            .collect();
        remaining.shuffle(&mut rand::rng());
        if remaining.is_empty() {
            remaining.extend_from_slice(images);
        }
    }
    remaining.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Catalog(Vec<PathBuf>);
    impl ImageCatalog for Catalog {
        fn images(&self, _: &Path) -> Result<Vec<PathBuf>, String> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn lifecycle_preserves_a_no_repeat_deck_and_reschedules() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle::new(Catalog(
            ["a.jpg", "b.jpg", "c.jpg"]
                .into_iter()
                .map(PathBuf::from)
                .collect(),
        ));
        let mut applied = Vec::new();
        let mut slideshow = lifecycle
            .start("monitor", "folder".into(), 60, now, |path| {
                applied.push(path.to_owned());
                Ok(())
            })
            .unwrap();
        lifecycle
            .advance(
                "monitor",
                &mut slideshow,
                now + Duration::from_secs(60),
                |path| {
                    applied.push(path.to_owned());
                    Ok(())
                },
            )
            .unwrap();
        assert_ne!(applied[0], applied[1]);
        assert_eq!(
            lifecycle.due(now + Duration::from_secs(119)),
            Vec::<String>::new()
        );
        assert_eq!(lifecycle.due(now + Duration::from_secs(120)), ["monitor"]);
    }

    #[test]
    fn lifecycle_retries_an_image_that_cannot_be_applied() {
        let mut lifecycle = Lifecycle::new(Catalog(vec!["a.jpg".into(), "b.jpg".into()]));
        let mut attempts = 0;
        let slideshow = lifecycle.start("monitor", "folder".into(), 60, Instant::now(), |_| {
            attempts += 1;
            if attempts == 1 {
                Err("bad image".into())
            } else {
                Ok(())
            }
        });
        assert!(slideshow.is_ok());
        assert_eq!(attempts, 2);
    }

    #[test]
    fn missed_deadline_is_reported_once() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle::new(Catalog(vec!["a.jpg".into()]));
        lifecycle
            .start("monitor", "folder".into(), 60, now, |_| Ok(()))
            .unwrap();
        assert_eq!(lifecycle.due(now + Duration::from_secs(600)), ["monitor"]);
        assert!(lifecycle.due(now + Duration::from_secs(601)).is_empty());
    }
}
