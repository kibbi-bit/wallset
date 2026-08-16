#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod instance;
mod monitor_registry;
mod slideshow;
mod startup;
mod wallpaper;
mod wallpaper_draft;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(instance) = instance::InstanceGuard::acquire()? else {
        return Ok(());
    };
    app::run(instance)
}
