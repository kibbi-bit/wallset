# Wallset

Wallset is a small Windows 11 wallpaper manager written in Rust with a Slint UI. It can assign a picture, solid color, or independently timed folder slideshow to each connected monitor individually.

> Wallset has been developed with substantial assistance from generative AI tools. All submitted code is reviewed and maintained by the project author.

## Features

- Independent picture, color, and slideshow configuration per physical monitor
- Slideshow ordering by newest date added or a no-repeat randomized deck, with custom intervals
- Shared Fill, Fit, Stretch, Center, or Tile positioning
- Monitor hot-plug detection with saved configuration restoration
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

The executable is written to `target\release\wallset.exe`.

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
