# Contributing to Wallset

## Before you start

Check the [GitHub issue tracker](https://github.com/kibbi-bit/wallset/issues) before beginning substantial work. Issues are the project's request and specification surface: use an existing issue when one covers the change, or open one to discuss a new feature, behavior change, or larger refactor before investing in an implementation. Small, self-contained corrections may be submitted directly when their purpose is clear.

Keep each contribution focused on one concern. Avoid including unrelated cleanup, dependency updates, or release work in the same change. If an issue has acceptance criteria, treat them as the definition of done and call out anything intentionally left for another issue.

Thank you for helping improve wallset!

## Development environment

Wallset targets Windows 11 and calls Windows APIs for monitor discovery, wallpaper management, single-instance activation, and launch-at-sign-in settings. Use a Windows development machine with the stable Rust toolchain installed.

Clone the repository and verify the starting point:

```powershell
cargo test --locked
cargo run
```

Running the application is an integration test against your real desktop. It may change monitor wallpapers, write configuration under `%APPDATA%\Wallset`, create cached color wallpapers under `%LOCALAPPDATA%\Wallset`, and modify the current user's launch-at-sign-in setting when that option is exercised. Use test wallpaper files and review those locations when testing persistence-related changes.

## Finding your way around

- `ui/app.slint` defines the windows, controls, and UI callbacks.
- `src/app.rs` connects the Slint UI to application behavior.
- `src/config.rs` owns persisted configuration.
- `src/monitor_registry.rs` reconciles connected and saved monitors.
- `src/wallpaper.rs` applies wallpaper modes through Windows APIs.
- `src/wallpaper_draft.rs` validates and converts editable UI state.
- `src/slideshow.rs` owns slideshow ordering and scheduling behavior.
- `src/startup.rs` manages launch at Windows sign-in.
- `src/instance.rs` enforces and activates the single running instance.

Read [`CONTEXT.md`](./CONTEXT.md) before changing domain behavior. Use its established terms—particularly *monitor*, *wallpaper mode*, *slideshow*, *wallpaper draft*, and *monitor registry*—in code, tests, issues, and documentation.

## Making changes

Preserve saved configuration compatibility unless an issue explicitly calls for a migration or breaking change. Missing wallpaper files and folders are retained as unavailable configuration rather than silently discarded, and saved monitors remain known while disconnected.

Keep platform-specific and `unsafe` Windows API code narrowly scoped. Document non-obvious safety assumptions, preserve ownership and cleanup requirements for Windows handles and allocated values, and test failure paths where they can be isolated from the operating system.

Add or update tests for observable behavior changes. Existing tests live beside their implementation modules and favor deterministic domain behavior over reliance on connected hardware. Keep slideshow tests deterministic, including ordering, retry, restoration, and no-repeat behavior.

For UI changes, verify the relevant states manually at normal Windows scaling. Check disabled and invalid states, empty or unavailable paths, disconnected monitors where applicable, and keyboard access to top-level actions. Keep the Slint attribution in the About dialog present and reachable.

## Required checks

Run the full project checks before submitting:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

Also exercise the changed behavior manually when it depends on Windows, Slint rendering, monitor topology, the filesystem, persistence, or the registry. In the contribution description, summarize the manual scenarios tested and mention any checks that could not be run.

## Submitting a contribution

Open a pull request with:

- a concise description of the problem and solution;
- a link to the relevant issue, when one exists;
- the automated and manual checks performed;
- screenshots for visible UI changes; and
- notes about configuration compatibility, new dependencies, assets, or Windows API behavior.

Keep commits reviewable and ensure generated build output from `target/` is not committed. Respond to review by updating the contribution or explaining the tradeoff when a different approach is intentional.

## Licensing and provenance

By submitting a contribution to Wallset, you agree that it is licensed under the project's [MIT License](./LICENSE).

Only submit code, images, fonts, icons, or other material that you have the right to license this way. If a contribution incorporates third-party material, identify its source and license and preserve all notices required by the upstream author. Generative-AI assistance does not remove the need to review provenance or possible copied material.

If dependencies or bundled assets change, review their redistribution terms and update [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md) and the asset provenance record in [`docs/licensing.md`](./docs/licensing.md) as needed. Do not assume Wallset's MIT License grants rights to third-party material.