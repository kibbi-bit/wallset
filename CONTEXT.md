# Wallset

Wallset manages the wallpaper assigned to each Windows monitor, including independently timed slideshows and settings retained while a monitor is disconnected.

## Language

**Monitor**:
A physical display identified by its stable Windows device path. A monitor may be connected or retained as a saved monitor while disconnected.
_Avoid_: Screen, display device

**Wallpaper mode**:
The configured source of a monitor's wallpaper: a picture, a solid color, or a slideshow.
_Avoid_: Wallpaper kind, source type

**Slideshow**:
A wallpaper mode that selects images from one folder by date added or using a persisted no-repeat deck, and advances on a per-monitor interval.
_Avoid_: Rotation, playlist

**Wallpaper draft**:
The editable wallpaper mode shown for the selected monitor before it is applied.
_Avoid_: Form state, pending settings

**Monitor registry**:
The coherent view formed by reconciling connected monitors with saved monitors.
_Avoid_: Display list, monitor cache
