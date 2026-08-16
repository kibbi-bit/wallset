use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use image::{ImageBuffer, ImageFormat, Rgb};
use windows::{
    Win32::{
        Foundation::RECT,
        System::Com::{
            CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
            CoUninitialize,
        },
        UI::Shell::{
            DESKTOP_WALLPAPER_POSITION, DWPOS_CENTER, DWPOS_FILL, DWPOS_FIT, DWPOS_STRETCH,
            DWPOS_TILE, DesktopWallpaper, IDesktopWallpaper,
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::{
    config::{AppConfig, FitMode, MonitorConfig, Paths, WallpaperMode},
    monitor_registry::MonitorRegistry,
    slideshow::{Lifecycle, Slideshow, is_supported_image},
};

pub use crate::monitor_registry::Monitor as MonitorInfo;

#[derive(Debug)]
pub enum Command {
    Apply {
        monitor_id: String,
        mode: WallpaperMode,
    },
    ApplyFit(FitMode),
    Refresh,
    Next(String),
    NextAll,
    SetLaunchAtLogin(bool),
    Stop,
}

#[derive(Clone, Debug)]
pub enum Event {
    State {
        monitors: Vec<MonitorInfo>,
        config: AppConfig,
    },
    Status(String),
    Busy(bool),
}

pub fn spawn(paths: Paths, config: AppConfig, event_tx: Sender<Event>) -> Sender<Command> {
    let (command_tx, command_rx) = mpsc::channel();
    thread::spawn(move || match WindowsWallpaper::new() {
        Ok(backend) => {
            let effects = RuntimeEffects {
                config_file: paths.config_file,
                event_tx,
            };
            Transitions::new(backend, effects, paths.color_cache, config).run(command_rx)
        }
        Err(error) => {
            let _ = event_tx.send(Event::Status(format!(
                "Windows wallpaper API unavailable: {error}"
            )));
        }
    });
    command_tx
}

trait TransitionEffects {
    fn now(&self) -> Instant;
    fn save(&self, config: &AppConfig) -> Result<(), String>;
    fn publish(&self, event: Event);
}

struct RuntimeEffects {
    config_file: PathBuf,
    event_tx: Sender<Event>,
}

impl TransitionEffects for RuntimeEffects {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn save(&self, config: &AppConfig) -> Result<(), String> {
        config
            .save(&self.config_file)
            .map_err(|error| format!("Cannot save settings: {error}"))
    }

    fn publish(&self, event: Event) {
        let _ = self.event_tx.send(event);
    }
}

trait WallpaperBackend {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, String>;
    fn set_wallpaper(&self, monitor_id: &str, path: &Path) -> Result<(), String>;
    fn set_fit(&self, fit: FitMode) -> Result<(), String>;
}

struct WindowsWallpaper {
    api: IDesktopWallpaper,
}

impl WindowsWallpaper {
    fn new() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|error| error.to_string())?;
            let api = match CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) {
                Ok(api) => api,
                Err(error) => {
                    CoUninitialize();
                    return Err(error.to_string());
                }
            };
            Ok(Self { api })
        }
    }
}

impl Drop for WindowsWallpaper {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn path_wide(value: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.as_os_str().encode_wide().chain(Some(0)).collect()
}

unsafe fn take_pwstr(value: PWSTR) -> Result<String, String> {
    let result = unsafe { value.to_string() }.map_err(|error| error.to_string());
    unsafe { CoTaskMemFree(Some(value.as_ptr().cast())) };
    result
}

impl WallpaperBackend for WindowsWallpaper {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        unsafe {
            let count = self
                .api
                .GetMonitorDevicePathCount()
                .map_err(|error| error.to_string())?;
            let mut monitors = Vec::with_capacity(count as usize);
            for index in 0..count {
                let id = take_pwstr(
                    self.api
                        .GetMonitorDevicePathAt(index)
                        .map_err(|error| error.to_string())?,
                )?;
                let id_wide = wide(&id);
                let RECT {
                    left,
                    top,
                    right,
                    bottom,
                } = self
                    .api
                    .GetMonitorRECT(PCWSTR(id_wide.as_ptr()))
                    .map_err(|error| error.to_string())?;
                let wallpaper = self
                    .api
                    .GetWallpaper(PCWSTR(id_wide.as_ptr()))
                    .ok()
                    .and_then(|value| take_pwstr(value).ok())
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from);
                let primary = left == 0 && top == 0;
                monitors.push(MonitorInfo {
                    id,
                    name: format!(
                        "Display {}{}",
                        index + 1,
                        if primary { " · Primary" } else { "" }
                    ),
                    detail: format!("{} × {}", right - left, bottom - top),
                    current_wallpaper: wallpaper,
                    available: true,
                });
            }
            Ok(monitors)
        }
    }

    fn set_wallpaper(&self, monitor_id: &str, path: &Path) -> Result<(), String> {
        let monitor = wide(monitor_id);
        let path = path_wide(path);
        unsafe {
            self.api
                .SetWallpaper(PCWSTR(monitor.as_ptr()), PCWSTR(path.as_ptr()))
                .map_err(|error| error.to_string())
        }
    }

    fn set_fit(&self, fit: FitMode) -> Result<(), String> {
        let position: DESKTOP_WALLPAPER_POSITION = match fit {
            FitMode::Fill => DWPOS_FILL,
            FitMode::Fit => DWPOS_FIT,
            FitMode::Stretch => DWPOS_STRETCH,
            FitMode::Center => DWPOS_CENTER,
            FitMode::Tile => DWPOS_TILE,
        };
        unsafe {
            self.api
                .SetPosition(position)
                .map_err(|error| error.to_string())
        }
    }
}

struct Transitions<B: WallpaperBackend, E: TransitionEffects> {
    backend: B,
    effects: E,
    color_cache: PathBuf,
    config: AppConfig,
    registry: MonitorRegistry,
    slideshows: Lifecycle,
}

impl<B: WallpaperBackend, E: TransitionEffects> Transitions<B, E> {
    fn new(backend: B, effects: E, color_cache: PathBuf, config: AppConfig) -> Self {
        Self {
            backend,
            effects,
            color_cache,
            config,
            registry: MonitorRegistry::default(),
            slideshows: Lifecycle::default(),
        }
    }

    fn run(mut self, command_rx: Receiver<Command>) {
        if let Err(error) = self.refresh(true, true) {
            self.status(error);
        }
        let mut next_monitor_poll = self.effects.now() + Duration::from_secs(5);
        loop {
            match command_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(command) => {
                    if !self.handle(command) {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            let now = self.effects.now();
            for id in self.slideshows.due(now) {
                self.advance(&id, false);
            }
            if now >= next_monitor_poll {
                if let Err(error) = self.refresh(false, false) {
                    self.status(error);
                }
                next_monitor_poll = now + Duration::from_secs(5);
            }
        }
    }

    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Apply { monitor_id, mode } => self.apply(monitor_id, mode),
            Command::ApplyFit(fit) => self.apply_fit(fit),
            Command::Refresh => {
                if let Err(error) = self.refresh(false, true) {
                    self.status(error);
                }
            }
            Command::Next(id) => self.advance(&id, true),
            Command::NextAll => {
                let ids: Vec<_> = self
                    .config
                    .monitors
                    .iter()
                    .filter(|(_, config)| matches!(config.mode, WallpaperMode::Slideshow { .. }))
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in ids {
                    self.advance(&id, false);
                }
                self.publish();
            }
            Command::SetLaunchAtLogin(enabled) => {
                self.config.launch_at_login = enabled;
                if let Err(error) = self.save_config() {
                    self.status(error);
                }
                self.publish();
            }
            Command::Stop => return false,
        }
        true
    }

    fn refresh(&mut self, restore_all: bool, force_publish: bool) -> Result<(), String> {
        let current = self.backend.monitors()?;
        let reconciliation = self.registry.reconcile(current);
        let restored: Vec<_> = if restore_all {
            self.config
                .monitors
                .keys()
                .filter(|id| self.registry.is_connected(id))
                .filter(|id| self.config.monitors.contains_key(*id))
                .cloned()
                .collect()
        } else {
            reconciliation
                .newly_connected
                .iter()
                .filter(|id| self.config.monitors.contains_key(*id))
                .cloned()
                .collect()
        };
        let restored_any = !restored.is_empty();
        for id in restored {
            if let Some(config) = self.config.monitors.get(&id).cloned() {
                self.apply_existing(&id, config);
            }
        }
        if force_publish || restore_all || reconciliation.topology_changed || restored_any {
            self.publish();
        }
        Ok(())
    }

    fn apply(&mut self, monitor_id: String, mode: WallpaperMode) {
        self.effects.publish(Event::Busy(true));
        let result = self.validate_and_apply(&monitor_id, mode);
        match result {
            Ok(()) => self.status("Wallpaper applied"),
            Err(error) => self.status(error),
        }
        self.effects.publish(Event::Busy(false));
        self.publish();
    }

    fn validate_and_apply(&mut self, monitor_id: &str, mode: WallpaperMode) -> Result<(), String> {
        if !self.registry.is_connected(monitor_id) {
            return Err("The selected monitor is disconnected".into());
        }
        let mut monitor_config = MonitorConfig {
            mode: mode.clone(),
            current_image: None,
            remaining_shuffle_deck: Vec::new(),
        };
        match &mode {
            WallpaperMode::Picture { path } => {
                if !path.is_file() || !is_supported_image(path) {
                    return Err("Choose an existing JPEG, PNG, or BMP image".into());
                }
                self.backend.set_wallpaper(monitor_id, path)?;
                monitor_config.current_image = Some(path.clone());
                self.slideshows.cancel(monitor_id);
            }
            WallpaperMode::Color { rgb } => {
                let path = self.color_file(*rgb)?;
                self.backend.set_wallpaper(monitor_id, &path)?;
                monitor_config.current_image = Some(path);
                self.slideshows.cancel(monitor_id);
            }
            WallpaperMode::Slideshow {
                folder,
                interval_seconds,
                order,
            } => {
                let backend = &self.backend;
                let slideshow = self.slideshows.start(
                    monitor_id,
                    folder.clone(),
                    *interval_seconds,
                    *order,
                    self.effects.now(),
                    |path| backend.set_wallpaper(monitor_id, path),
                )?;
                monitor_config.current_image = slideshow.current_image;
                monitor_config.remaining_shuffle_deck = slideshow.remaining_deck;
            }
        }
        self.config
            .monitors
            .insert(monitor_id.to_string(), monitor_config);
        self.save_config()
    }

    fn apply_existing(&mut self, monitor_id: &str, config: MonitorConfig) {
        match config.mode.clone() {
            WallpaperMode::Picture { path } => {
                if path.is_file() {
                    if let Err(error) = self.backend.set_wallpaper(monitor_id, &path) {
                        self.status(error);
                    }
                } else {
                    self.status(format!("Saved wallpaper is missing: {}", path.display()));
                }
            }
            WallpaperMode::Color { rgb } => match self.color_file(rgb) {
                Ok(path) => {
                    if let Err(error) = self.backend.set_wallpaper(monitor_id, &path) {
                        self.status(error);
                    }
                }
                Err(error) => self.status(error),
            },
            WallpaperMode::Slideshow {
                folder,
                interval_seconds,
                order,
            } => {
                let mut slideshow = Slideshow {
                    folder,
                    interval_seconds,
                    order,
                    current_image: config.current_image,
                    remaining_deck: config.remaining_shuffle_deck,
                };
                let backend = &self.backend;
                match self.slideshows.restore(
                    monitor_id,
                    &mut slideshow,
                    self.effects.now(),
                    |path| backend.set_wallpaper(monitor_id, path),
                ) {
                    Ok(()) => {
                        if let Some(saved) = self.config.monitors.get_mut(monitor_id) {
                            saved.current_image = slideshow.current_image;
                            saved.remaining_shuffle_deck = slideshow.remaining_deck;
                        }
                        if let Err(error) = self.save_config() {
                            self.status(error);
                        }
                    }
                    Err(error) => self.status(error),
                }
            }
        }
    }

    fn advance(&mut self, monitor_id: &str, publish: bool) {
        if !self.registry.is_connected(monitor_id) {
            return;
        }
        let Some(mut config) = self.config.monitors.remove(monitor_id) else {
            return;
        };
        let result = if let WallpaperMode::Slideshow {
            folder,
            interval_seconds,
            order,
        } = &config.mode
        {
            let mut slideshow = Slideshow {
                folder: folder.clone(),
                interval_seconds: *interval_seconds,
                order: *order,
                current_image: config.current_image.clone(),
                remaining_deck: std::mem::take(&mut config.remaining_shuffle_deck),
            };
            let backend = &self.backend;
            let result =
                self.slideshows
                    .advance(monitor_id, &mut slideshow, self.effects.now(), |path| {
                        backend.set_wallpaper(monitor_id, path)
                    });
            config.current_image = slideshow.current_image;
            config.remaining_shuffle_deck = slideshow.remaining_deck;
            result
        } else {
            Err("The selected monitor is not using a slideshow".to_string())
        };
        self.config.monitors.insert(monitor_id.to_string(), config);
        if let Err(error) = result {
            self.slideshows.cancel(monitor_id);
            self.status(error);
        } else if publish {
            self.status("Wallpaper advanced");
        }
        if let Err(error) = self.save_config() {
            self.status(error);
        }
        if publish {
            self.publish();
        }
    }

    fn apply_fit(&mut self, fit: FitMode) {
        match self.backend.set_fit(fit) {
            Ok(()) => {
                self.config.shared_fit = fit;
                if let Err(error) = self.save_config() {
                    self.status(error);
                } else {
                    self.status("Image fit applied to all monitors");
                }
            }
            Err(error) => self.status(error),
        }
        self.publish();
    }

    fn color_file(&self, rgb: [u8; 3]) -> Result<PathBuf, String> {
        let path = self
            .color_cache
            .join(format!("{:02x}{:02x}{:02x}.bmp", rgb[0], rgb[1], rgb[2]));
        if !path.exists() {
            let image = ImageBuffer::from_pixel(1, 1, Rgb(rgb));
            image
                .save_with_format(&path, ImageFormat::Bmp)
                .map_err(|error| format!("Cannot create color wallpaper: {error}"))?;
        }
        Ok(path)
    }

    fn save_config(&self) -> Result<(), String> {
        self.effects.save(&self.config)
    }

    fn publish(&self) {
        let monitors = self.registry.view(self.config.monitors.keys());
        self.effects.publish(Event::State {
            monitors,
            config: self.config.clone(),
        });
    }

    fn status(&self, message: impl Into<String>) {
        self.effects.publish(Event::Status(message.into()));
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, path::Path};

    use super::*;

    struct Backend;

    impl WallpaperBackend for Backend {
        fn monitors(&self) -> Result<Vec<MonitorInfo>, String> {
            Ok(Vec::new())
        }

        fn set_wallpaper(&self, _: &str, _: &Path) -> Result<(), String> {
            Ok(())
        }

        fn set_fit(&self, _: FitMode) -> Result<(), String> {
            Ok(())
        }
    }

    struct Effects {
        now: Instant,
        fail_save: bool,
        events: RefCell<Vec<Event>>,
    }

    impl TransitionEffects for Effects {
        fn now(&self) -> Instant {
            self.now
        }

        fn save(&self, _: &AppConfig) -> Result<(), String> {
            if self.fail_save {
                Err("Cannot save settings: disk full".into())
            } else {
                Ok(())
            }
        }

        fn publish(&self, event: Event) {
            self.events.borrow_mut().push(event);
        }
    }

    #[test]
    fn command_interface_keeps_truthful_state_when_persistence_fails() {
        let effects = Effects {
            now: Instant::now(),
            fail_save: true,
            events: RefCell::new(Vec::new()),
        };
        let mut transitions =
            Transitions::new(Backend, effects, PathBuf::new(), AppConfig::default());

        assert!(transitions.handle(Command::ApplyFit(FitMode::Tile)));

        assert_eq!(transitions.config.shared_fit, FitMode::Tile);
        let events = transitions.effects.events.borrow();
        assert!(
            events.iter().any(
                |event| matches!(event, Event::Status(message) if message.contains("disk full"))
            )
        );
        assert!(events.iter().any(|event| matches!(event, Event::State { config, .. } if config.shared_fit == FitMode::Tile)));
    }

    #[test]
    fn stop_command_ends_the_transition_loop() {
        let effects = Effects {
            now: Instant::now(),
            fail_save: false,
            events: RefCell::new(Vec::new()),
        };
        let mut transitions =
            Transitions::new(Backend, effects, PathBuf::new(), AppConfig::default());
        assert!(!transitions.handle(Command::Stop));
    }
}
