use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

use slint::{CloseRequestResponse, ComponentHandle, ModelRc, VecModel};

use crate::{
    AboutWindow, AppTray, AppWindow, MonitorRow, WallpaperKind,
    config::{AppConfig, FitMode, Paths, SlideshowOrder, WallpaperMode},
    instance::InstanceGuard,
    startup,
    wallpaper::{self, Command, Event, MonitorInfo},
    wallpaper_draft::{IntervalUnit, WallpaperDraft},
};

#[derive(Default)]
struct UiState {
    monitors: Vec<MonitorInfo>,
    config: AppConfig,
    selected_id: Option<String>,
}

pub fn run(instance: InstanceGuard) -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::discover()?;
    let mut config = AppConfig::load(&paths.config_file).unwrap_or_else(|error| {
        eprintln!("Could not load Wallset configuration: {error}");
        AppConfig::default()
    });
    config.launch_at_login = startup::is_enabled();

    let window = AppWindow::new()?;
    let about = AboutWindow::new()?;
    let tray = AppTray::new()?;
    about.set_wallset_version(env!("CARGO_PKG_VERSION").into());
    window.set_fit_index(config.shared_fit.index());
    window.set_launch_at_login(config.launch_at_login);

    let state = Arc::new(Mutex::new(UiState {
        config: config.clone(),
        ..UiState::default()
    }));
    let (event_tx, event_rx) = mpsc::channel();
    let command_tx = wallpaper::spawn(paths, config, event_tx);

    wire_window(&window, Arc::clone(&state), command_tx.clone());
    wire_about(&window, &about, &tray);
    wire_tray(&window, &tray, command_tx.clone());
    forward_events(&window, Arc::clone(&state), event_rx);
    watch_activation(&window, instance.activation_event());

    window
        .window()
        .on_close_requested(|| CloseRequestResponse::HideWindow);
    tray.show()?;
    if !std::env::args().any(|argument| argument == "--background") {
        window.show()?;
    }
    slint::run_event_loop()?;
    let _ = command_tx.send(Command::Stop);
    drop(instance);
    Ok(())
}

fn wire_about(window: &AppWindow, about: &AboutWindow, tray: &AppTray) {
    about
        .window()
        .on_close_requested(|| CloseRequestResponse::HideWindow);

    let weak = about.as_weak();
    window.on_about(move || {
        if let Some(about) = weak.upgrade() {
            let _ = about.show();
        }
    });

    let weak = about.as_weak();
    tray.on_about(move || {
        if let Some(about) = weak.upgrade() {
            let _ = about.show();
        }
    });

    let weak = about.as_weak();
    about.on_dismiss(move || {
        if let Some(about) = weak.upgrade() {
            let _ = about.hide();
        }
    });
}

fn watch_activation(window: &AppWindow, event: windows::Win32::Foundation::HANDLE) {
    use windows::Win32::{
        Foundation::{HANDLE, WAIT_OBJECT_0},
        System::Threading::{INFINITE, WaitForSingleObject},
    };
    let weak = window.as_weak();
    let event_value = event.0 as usize;
    std::thread::spawn(move || {
        loop {
            let event = HANDLE(event_value as *mut _);
            if unsafe { WaitForSingleObject(event, INFINITE) } != WAIT_OBJECT_0 {
                break;
            }
            if weak
                .upgrade_in_event_loop(|window| {
                    let _ = window.show();
                })
                .is_err()
            {
                break;
            }
        }
    });
}

fn wire_window(window: &AppWindow, state: Arc<Mutex<UiState>>, command_tx: mpsc::Sender<Command>) {
    let weak = window.as_weak();
    let state_for_select = Arc::clone(&state);
    window.on_select_monitor(move |index| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut state = state_for_select.lock().expect("UI state poisoned");
        let Some(monitor) = state.monitors.get(index as usize) else {
            return;
        };
        state.selected_id = Some(monitor.id.clone());
        window.set_selected_index(index);
        load_selected_draft(&window, &state);
    });

    let weak = window.as_weak();
    let state_for_picture = Arc::clone(&state);
    window.on_choose_picture(move || {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Choose wallpaper")
            .add_filter("Wallpaper images", &["jpg", "jpeg", "png", "bmp"])
            .pick_file()
            && let Some(window) = weak.upgrade()
        {
            window.set_draft_picture_path(path.to_string_lossy().into_owned().into());
            set_preview_path(&window, &path);
            update_can_apply(
                &window,
                &state_for_picture.lock().expect("UI state poisoned"),
            );
        }
    });

    let weak = window.as_weak();
    let state_for_folder = Arc::clone(&state);
    window.on_choose_folder(move || {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Choose slideshow folder")
            .pick_folder()
            && let Some(window) = weak.upgrade()
        {
            window.set_draft_slideshow_folder(path.to_string_lossy().into_owned().into());
            update_can_apply(
                &window,
                &state_for_folder.lock().expect("UI state poisoned"),
            );
        }
    });

    let weak = window.as_weak();
    let state_for_draft = Arc::clone(&state);
    window.on_draft_changed(move || {
        if let Some(window) = weak.upgrade() {
            update_can_apply(&window, &state_for_draft.lock().expect("UI state poisoned"));
        }
    });

    let weak = window.as_weak();
    window.on_color_edited(move |value| {
        if let Some(rgb) = (WallpaperDraft::Color {
            value: value.to_string(),
        })
        .color_rgb()
            && let Some(window) = weak.upgrade()
        {
            window.set_preview_color(slint::Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]));
        }
    });

    let weak = window.as_weak();
    let state_for_apply = Arc::clone(&state);
    let tx = command_tx.clone();
    window.on_apply_monitor(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = state_for_apply.lock().expect("UI state poisoned");
        let Some(monitor_id) = state.selected_id.clone() else {
            window.set_status_text("Select a monitor first".into());
            return;
        };
        match mode_from_draft(&window) {
            Ok(mode) if state.config.monitors.get(&monitor_id).map(|c| &c.mode) != Some(&mode) => {
                let _ = tx.send(Command::Apply { monitor_id, mode });
            }
            Ok(_) => window.set_can_apply(false),
            Err(error) => window.set_status_text(error.into()),
        }
    });

    let weak = window.as_weak();
    let tx = command_tx.clone();
    window.on_apply_fit(move || {
        if let Some(window) = weak.upgrade() {
            let _ = tx.send(Command::ApplyFit(FitMode::from_index(
                window.get_fit_index(),
            )));
        }
    });

    let tx = command_tx.clone();
    window.on_refresh_monitors(move || {
        let _ = tx.send(Command::Refresh);
    });

    let state_for_next = Arc::clone(&state);
    let tx = command_tx.clone();
    window.on_next_wallpaper(move || {
        if let Some(id) = state_for_next
            .lock()
            .expect("UI state poisoned")
            .selected_id
            .clone()
        {
            let _ = tx.send(Command::Next(id));
        }
    });

    let weak = window.as_weak();
    let tx = command_tx;
    window.on_startup_changed(move |enabled| match startup::set_enabled(enabled) {
        Ok(()) => {
            let _ = tx.send(Command::SetLaunchAtLogin(enabled));
        }
        Err(error) => {
            if let Some(window) = weak.upgrade() {
                window.set_launch_at_login(!enabled);
                window.set_status_text(format!("Could not update startup setting: {error}").into());
            }
        }
    });
}

fn wire_tray(window: &AppWindow, tray: &AppTray, command_tx: mpsc::Sender<Command>) {
    let weak = window.as_weak();
    tray.on_open(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.show();
        }
    });
    let weak = window.as_weak();
    tray.on_icon_clicked(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.show();
        }
    });

    let tx = command_tx.clone();
    tray.on_next_all(move || {
        let _ = tx.send(Command::NextAll);
    });

    let tx = command_tx;
    let tray_weak = tray.as_weak();
    tray.on_quit(move || {
        let _ = tx.send(Command::Stop);
        if let Some(tray) = tray_weak.upgrade() {
            let _ = tray.hide();
        }
        let _ = slint::quit_event_loop();
    });
}

fn forward_events(window: &AppWindow, state: Arc<Mutex<UiState>>, event_rx: mpsc::Receiver<Event>) {
    let weak = window.as_weak();
    std::thread::spawn(move || {
        while let Ok(event) = event_rx.recv() {
            let state = Arc::clone(&state);
            let weak = weak.clone();
            let _ = weak.upgrade_in_event_loop(move |window| match event {
                Event::State { monitors, config } => {
                    let mut state = state.lock().expect("UI state poisoned");
                    let old_selection = state.selected_id.clone();
                    state.monitors = monitors;
                    state.config = config;
                    state.selected_id = old_selection
                        .filter(|id| state.monitors.iter().any(|monitor| &monitor.id == id))
                        .or_else(|| state.monitors.first().map(|monitor| monitor.id.clone()));
                    update_monitor_model(&window, &state);
                    load_selected_draft(&window, &state);
                    window.set_fit_index(state.config.shared_fit.index());
                    window.set_launch_at_login(state.config.launch_at_login);
                }
                Event::Status(message) => window.set_status_text(message.into()),
                Event::Busy(busy) => window.set_busy(busy),
            });
        }
    });
}

fn update_monitor_model(window: &AppWindow, state: &UiState) {
    let rows: Vec<MonitorRow> = state
        .monitors
        .iter()
        .map(|monitor| MonitorRow {
            id: monitor.id.clone().into(),
            name: monitor.name.clone().into(),
            detail: monitor.detail.clone().into(),
            managed: state.config.monitors.contains_key(&monitor.id),
            available: monitor.available,
        })
        .collect();
    window.set_monitors(ModelRc::new(VecModel::from(rows)));
    let selected = state
        .selected_id
        .as_ref()
        .and_then(|id| state.monitors.iter().position(|monitor| &monitor.id == id))
        .unwrap_or(0);
    window.set_selected_index(selected as i32);
}

fn load_selected_draft(window: &AppWindow, state: &UiState) {
    let Some(id) = &state.selected_id else {
        render_draft(window, &WallpaperDraft::Current { path: None });
        window.set_can_apply(false);
        return;
    };
    if let Some(config) = state.config.monitors.get(id) {
        render_draft(window, &WallpaperDraft::from_mode(&config.mode));
    } else {
        let path = state
            .monitors
            .iter()
            .find(|monitor| &monitor.id == id)
            .and_then(|monitor| monitor.current_wallpaper.clone());
        render_draft(window, &WallpaperDraft::Current { path });
    }
    update_can_apply(window, state);
}

fn update_can_apply(window: &AppWindow, state: &UiState) {
    let saved = state
        .selected_id
        .as_ref()
        .and_then(|id| state.config.monitors.get(id))
        .map(|config| &config.mode);
    let can_apply =
        draft_from_window(window).is_applyable_against(saved) && state.selected_id.is_some();
    window.set_can_apply(can_apply);
}

fn render_draft(window: &AppWindow, draft: &WallpaperDraft) {
    match draft {
        WallpaperDraft::Current { path } => {
            window.set_draft_kind(WallpaperKind::Current);
            window.set_draft_slideshow_order(0);
            window.set_current_path(
                path.as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default()
                    .into(),
            );
            if let Some(path) = path {
                set_preview_path(window, path);
            } else {
                window.set_preview_image(slint::Image::default());
            }
        }
        WallpaperDraft::Picture { path } => {
            window.set_draft_kind(WallpaperKind::Picture);
            window.set_draft_slideshow_order(0);
            window.set_draft_picture_path(path.to_string_lossy().into_owned().into());
            set_preview_path(window, path);
        }
        WallpaperDraft::Color { value } => {
            window.set_draft_kind(WallpaperKind::Color);
            window.set_draft_slideshow_order(0);
            window.set_draft_color(value.clone().into());
            if let Some(rgb) = draft.color_rgb() {
                window.set_preview_color(slint::Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]));
            }
        }
        WallpaperDraft::Slideshow {
            folder,
            interval_value,
            interval_unit,
            order,
        } => {
            window.set_draft_kind(WallpaperKind::Slideshow);
            window.set_draft_slideshow_folder(folder.to_string_lossy().into_owned().into());
            window.set_draft_interval_value(interval_value.clone().into());
            window.set_draft_interval_unit(match interval_unit {
                IntervalUnit::Minutes => 0,
                IntervalUnit::Hours => 1,
            });
            window.set_draft_slideshow_order(match order {
                SlideshowOrder::DateAdded => 0,
                SlideshowOrder::Random => 1,
            });
        }
    }
}

fn set_preview_path(window: &AppWindow, path: &std::path::Path) {
    window.set_preview_image(slint::Image::load_from_path(path).unwrap_or_default());
}

fn mode_from_draft(window: &AppWindow) -> Result<WallpaperMode, String> {
    draft_from_window(window).apply_intent()
}

fn draft_from_window(window: &AppWindow) -> WallpaperDraft {
    match window.get_draft_kind() {
        WallpaperKind::Picture => WallpaperDraft::Picture {
            path: PathBuf::from(window.get_draft_picture_path().as_str()),
        },
        WallpaperKind::Color => WallpaperDraft::Color {
            value: window.get_draft_color().to_string(),
        },
        WallpaperKind::Slideshow => WallpaperDraft::Slideshow {
            folder: PathBuf::from(window.get_draft_slideshow_folder().as_str()),
            interval_value: window.get_draft_interval_value().to_string(),
            interval_unit: if window.get_draft_interval_unit() == 1 {
                IntervalUnit::Hours
            } else {
                IntervalUnit::Minutes
            },
            order: if window.get_draft_slideshow_order() == 1 {
                SlideshowOrder::Random
            } else {
                SlideshowOrder::DateAdded
            },
        },
        WallpaperKind::Current => WallpaperDraft::Current { path: None },
    }
}
