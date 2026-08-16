# Wallset

Wallset is a small Windows 11 wallpaper manager written in Rust with a Slint UI. It can assign a picture, solid color, or independently timed folder slideshow to each connected monitor.

## Features

- Independent picture, color, and slideshow configuration per physical monitor
- Slideshow ordering by newest date added or a no-repeat randomized deck, with custom minute or hour intervals
- Shared Fill, Fit, Stretch, Center, or Tile positioning
- Monitor hot-plug detection with saved configuration restoration
- Native Slint system tray; closing the window keeps slideshows running
- Optional launch at Windows sign-in
- No registry wallpaper hacks: monitor wallpapers are applied through `IDesktopWallpaper`

## Build and run

Install the stable Rust toolchain on Windows, then run:

```powershell
cargo run
```

For an optimized executable:

```powershell
cargo build --release
```

The executable is written to `target\release\wallset.exe`. This personal MVP does not include an installer or code signing.

## Using Wallset

1. Select a display from the left side of the window.
2. Choose Picture, Color, or Slideshow.
3. Select an image/folder or enter a `#RRGGBB` color. Slideshow folders are scanned at their top level for JPEG, PNG, and BMP files.
4. Choose an order and interval for a slideshow, then press **Apply**.
5. Repeat for other monitors. The Image fit control is intentionally shared because Windows exposes it as a desktop-wide setting.

Closing the window hides it in the notification area. Use the tray menu to reopen Wallset, advance all active slideshows, or quit. Quitting stops slideshow changes; saved settings resume when Wallset is launched again.

## Data locations

- Configuration: `%APPDATA%\Wallset\config\config.json`
- Generated color wallpapers: `%LOCALAPPDATA%\Wallset\cache\colors`

Wallset keeps configurations for disconnected monitors and reapplies them when Windows reports the same stable monitor device path again. Missing files and folders remain configured but are reported as unavailable rather than silently discarded.

## Development checks

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```
