use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use rand::seq::SliceRandom;

use crate::config::SlideshowOrder;

#[derive(Clone, Debug)]
pub struct ImageFile {
    path: PathBuf,
    added: Option<SystemTime>,
}

impl ImageFile {
    #[cfg(test)]
    fn new(path: impl Into<PathBuf>, added: Option<SystemTime>) -> Self {
        Self {
            path: path.into(),
            added,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Slideshow {
    pub folder: PathBuf,
    pub interval_seconds: u64,
    pub order: SlideshowOrder,
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
        order: SlideshowOrder,
        now: Instant,
        mut apply: impl FnMut(&Path) -> Result<(), String>,
    ) -> Result<Slideshow, String> {
        if interval_seconds < 60 {
            return Err("The minimum slideshow interval is one minute".into());
        }
        let mut slideshow = Slideshow {
            folder,
            interval_seconds,
            order,
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
        current_wallpaper: Option<&Path>,
        now: Instant,
        mut apply: impl FnMut(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let images = self.catalog.images(&slideshow.folder)?;
        if let Some(current) = current_wallpaper.and_then(|current| {
            images
                .iter()
                .find(|image| windows_paths_equal(&image.path, current))
                .map(|image| image.path.clone())
        }) {
            slideshow.current_image = Some(current.clone());
            slideshow
                .remaining_deck
                .retain(|path| !windows_paths_equal(path, &current));
        } else {
            self.advance_from_images(slideshow, images, &mut apply)?;
        }
        self.schedule(monitor_id, now, slideshow.interval_seconds);
        Ok(())
    }

    pub fn reconfigure(
        &mut self,
        monitor_id: &str,
        slideshow: &mut Slideshow,
        now: Instant,
    ) -> Result<(), String> {
        let images = self.catalog.images(&slideshow.folder)?;
        if images.is_empty() {
            return Err("The slideshow folder is empty".into());
        }
        let available: HashSet<&Path> = images.iter().map(|image| image.path.as_path()).collect();
        slideshow
            .remaining_deck
            .retain(|path| available.contains(path.as_path()));
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
        self.advance_from_images(slideshow, images, apply)
    }

    fn advance_from_images(
        &self,
        slideshow: &mut Slideshow,
        images: Vec<ImageFile>,
        apply: &mut impl FnMut(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        if images.is_empty() {
            return Err("The slideshow folder is empty".into());
        }
        match slideshow.order {
            SlideshowOrder::DateAdded => {
                for next in date_added_candidates(images, slideshow.current_image.as_deref()) {
                    if apply(&next).is_ok() {
                        slideshow.current_image = Some(next);
                        return Ok(());
                    }
                }
            }
            SlideshowOrder::Random => {
                for _ in 0..images.len() {
                    let Some(next) = next_random_image(
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
            }
        }
        Err("No slideshow image could be applied".into())
    }
}

pub(crate) fn windows_paths_equal(left: &Path, right: &Path) -> bool {
    left.components()
        .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
        .eq(right
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_lowercase()))
}

pub trait ImageCatalog {
    fn images(&self, folder: &Path) -> Result<Vec<ImageFile>, String>;
}

pub struct FileSystemCatalog;

impl ImageCatalog for FileSystemCatalog {
    fn images(&self, folder: &Path) -> Result<Vec<ImageFile>, String> {
        if !folder.is_dir() {
            return Err("Choose an existing slideshow folder".into());
        }
        let entries =
            fs::read_dir(folder).map_err(|error| format!("Cannot read folder: {error}"))?;
        let mut images = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_supported_image(&path) {
                let added = entry
                    .metadata()
                    .ok()
                    .and_then(|metadata| metadata.created().or_else(|_| metadata.modified()).ok());
                images.push(ImageFile { path, added });
            }
        }
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

fn next_random_image(
    images: &[ImageFile],
    current: Option<&Path>,
    remaining: &mut Vec<PathBuf>,
) -> Option<PathBuf> {
    let available: HashSet<&Path> = images.iter().map(|image| image.path.as_path()).collect();
    remaining.retain(|path| available.contains(path.as_path()));
    if remaining.is_empty() {
        *remaining = images
            .iter()
            .map(|image| &image.path)
            .filter(|path| Some(path.as_path()) != current)
            .cloned()
            .collect();
        remaining.shuffle(&mut rand::rng());
        if remaining.is_empty() {
            remaining.extend(images.iter().map(|image| image.path.clone()));
        }
    }
    remaining.pop()
}

fn date_added_candidates(mut images: Vec<ImageFile>, current: Option<&Path>) -> Vec<PathBuf> {
    images.sort_by(|left, right| {
        right.added.cmp(&left.added).then_with(|| {
            left.path
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.path.to_string_lossy().to_lowercase())
        })
    });
    let mut paths: Vec<_> = images.into_iter().map(|image| image.path).collect();
    if let Some(position) = paths
        .iter()
        .position(|path| Some(path.as_path()) == current)
    {
        let length = paths.len();
        paths.rotate_left((position + 1) % length);
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Catalog(Vec<ImageFile>);
    impl ImageCatalog for Catalog {
        fn images(&self, _: &Path) -> Result<Vec<ImageFile>, String> {
            Ok(self.0.clone())
        }
    }

    fn catalog(paths: &[&str]) -> Catalog {
        Catalog(
            paths
                .iter()
                .map(|path| ImageFile::new(*path, None))
                .collect(),
        )
    }

    #[test]
    fn lifecycle_preserves_a_no_repeat_deck_and_reschedules() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle::new(catalog(&["a.jpg", "b.jpg", "c.jpg"]));
        let mut applied = Vec::new();
        let mut slideshow = lifecycle
            .start(
                "monitor",
                "folder".into(),
                60,
                SlideshowOrder::Random,
                now,
                |path| {
                    applied.push(path.to_owned());
                    Ok(())
                },
            )
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
        let mut lifecycle = Lifecycle::new(catalog(&["a.jpg", "b.jpg"]));
        let mut attempts = 0;
        let slideshow = lifecycle.start(
            "monitor",
            "folder".into(),
            60,
            SlideshowOrder::Random,
            Instant::now(),
            |_| {
                attempts += 1;
                if attempts == 1 {
                    Err("bad image".into())
                } else {
                    Ok(())
                }
            },
        );
        assert!(slideshow.is_ok());
        assert_eq!(attempts, 2);
    }

    #[test]
    fn restore_retains_current_date_added_image_and_reschedules() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle::new(catalog(&["folder/current.jpg", "folder/next.jpg"]));
        let mut slideshow = Slideshow {
            folder: "folder".into(),
            interval_seconds: 60,
            order: SlideshowOrder::DateAdded,
            current_image: Some("folder/next.jpg".into()),
            remaining_deck: Vec::new(),
        };
        let mut applied = Vec::new();

        lifecycle
            .restore(
                "monitor",
                &mut slideshow,
                Some(Path::new("FOLDER\\CURRENT.JPG")),
                now,
                |path| {
                    applied.push(path.to_owned());
                    Ok(())
                },
            )
            .unwrap();

        assert!(applied.is_empty());
        assert_eq!(slideshow.current_image, Some("folder/current.jpg".into()));
        assert!(lifecycle.due(now + Duration::from_secs(59)).is_empty());
        assert_eq!(lifecycle.due(now + Duration::from_secs(60)), ["monitor"]);
    }

    #[test]
    fn restore_retains_random_image_without_consuming_or_rebuilding_deck() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle::new(catalog(&[
            "folder/current.jpg",
            "folder/next.jpg",
            "folder/later.jpg",
        ]));
        let mut slideshow = Slideshow {
            folder: "folder".into(),
            interval_seconds: 60,
            order: SlideshowOrder::Random,
            current_image: Some("folder/old.jpg".into()),
            remaining_deck: vec![
                "folder/later.jpg".into(),
                "folder/current.jpg".into(),
                "folder/next.jpg".into(),
            ],
        };

        lifecycle
            .restore(
                "monitor",
                &mut slideshow,
                Some(Path::new("folder/current.jpg")),
                now,
                |_| panic!("retained wallpaper must not be reapplied"),
            )
            .unwrap();

        assert_eq!(slideshow.current_image, Some("folder/current.jpg".into()));
        assert_eq!(
            slideshow.remaining_deck,
            ["folder/later.jpg", "folder/next.jpg"].map(PathBuf::from)
        );
    }

    #[test]
    fn restore_applies_a_valid_image_when_current_wallpaper_is_outside_catalog() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle::new(catalog(&["folder/current.jpg", "folder/next.jpg"]));
        let mut slideshow = Slideshow {
            folder: "folder".into(),
            interval_seconds: 60,
            order: SlideshowOrder::DateAdded,
            current_image: Some("folder/current.jpg".into()),
            remaining_deck: Vec::new(),
        };
        let mut applied = Vec::new();

        lifecycle
            .restore(
                "monitor",
                &mut slideshow,
                Some(Path::new("elsewhere/wallpaper.jpg")),
                now,
                |path| {
                    applied.push(path.to_owned());
                    Ok(())
                },
            )
            .unwrap();

        assert_eq!(applied, [PathBuf::from("folder/next.jpg")]);
        assert_eq!(slideshow.current_image, Some("folder/next.jpg".into()));
    }

    #[test]
    fn missed_deadline_is_reported_once() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle::new(catalog(&["a.jpg"]));
        lifecycle
            .start(
                "monitor",
                "folder".into(),
                60,
                SlideshowOrder::Random,
                now,
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(lifecycle.due(now + Duration::from_secs(600)), ["monitor"]);
        assert!(lifecycle.due(now + Duration::from_secs(601)).is_empty());
    }

    #[test]
    fn date_added_advances_newest_to_oldest_and_wraps() {
        let base = SystemTime::UNIX_EPOCH;
        let mut lifecycle = Lifecycle::new(Catalog(vec![
            ImageFile::new("old.jpg", Some(base + Duration::from_secs(1))),
            ImageFile::new("new.jpg", Some(base + Duration::from_secs(3))),
            ImageFile::new("middle.jpg", Some(base + Duration::from_secs(2))),
        ]));
        let now = Instant::now();
        let mut applied = Vec::new();
        let mut slideshow = lifecycle
            .start(
                "monitor",
                "folder".into(),
                60,
                SlideshowOrder::DateAdded,
                now,
                |path| {
                    applied.push(path.to_owned());
                    Ok(())
                },
            )
            .unwrap();
        for step in 1..=3 {
            lifecycle
                .advance(
                    "monitor",
                    &mut slideshow,
                    now + Duration::from_secs(60 * step),
                    |path| {
                        applied.push(path.to_owned());
                        Ok(())
                    },
                )
                .unwrap();
        }
        assert_eq!(
            applied,
            ["new.jpg", "middle.jpg", "old.jpg", "new.jpg"].map(PathBuf::from)
        );
    }

    #[test]
    fn date_added_uses_path_as_a_deterministic_tie_breaker() {
        let candidates = date_added_candidates(
            vec![ImageFile::new("b.jpg", None), ImageFile::new("A.jpg", None)],
            None,
        );
        assert_eq!(candidates, ["A.jpg", "b.jpg"].map(PathBuf::from));
    }

    #[test]
    fn date_added_resumes_at_newest_when_current_image_is_missing() {
        let base = SystemTime::UNIX_EPOCH;
        let candidates = date_added_candidates(
            vec![
                ImageFile::new("old.jpg", Some(base + Duration::from_secs(1))),
                ImageFile::new("new.jpg", Some(base + Duration::from_secs(2))),
            ],
            Some(Path::new("removed.jpg")),
        );
        assert_eq!(candidates, ["new.jpg", "old.jpg"].map(PathBuf::from));
    }

    #[test]
    fn date_added_retries_the_next_image_when_apply_fails() {
        let base = SystemTime::UNIX_EPOCH;
        let mut lifecycle = Lifecycle::new(Catalog(vec![
            ImageFile::new("old.jpg", Some(base + Duration::from_secs(1))),
            ImageFile::new("new.jpg", Some(base + Duration::from_secs(2))),
        ]));
        let mut attempts = Vec::new();
        let slideshow = lifecycle
            .start(
                "monitor",
                "folder".into(),
                60,
                SlideshowOrder::DateAdded,
                Instant::now(),
                |path| {
                    attempts.push(path.to_owned());
                    if path == Path::new("new.jpg") {
                        Err("bad image".into())
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap();
        assert_eq!(attempts, ["new.jpg", "old.jpg"].map(PathBuf::from));
        assert_eq!(slideshow.current_image, Some("old.jpg".into()));
    }
}
