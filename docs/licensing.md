# Licensing and distribution

Wallset's source code and original assets are licensed under the root [MIT License](../LICENSE). This record covers the repository state used to build a release; it is project hygiene, not legal advice.

## Slint licensing choice

Wallset 0.1 uses Slint 1.17.1 under the **Slint Royalty-free Desktop, Mobile, and Web Applications License**, not under Slint's GPLv3 option. Wallset is a Windows desktop application and keeps its own source under MIT. The About Wallset dialog is reachable from the application's top-level menu and contains Slint's `AboutSlint` widget, satisfying the selected license's in-application attribution route.

Before publishing an executable, verify the terms shipped with the exact locked Slint version and confirm that the attribution remains present and reachable. Do not distribute Wallset for an embedded system under this licensing choice.

## Dependency notices

Rust dependencies are locked by `Cargo.lock`. Their declared licenses and upstream license files were reviewed for the Windows release dependency graph. The dependencies use permissive licenses, with Slint's custom royalty-free license as the intentional exception documented above. The hand-maintained [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md) records that exception and is packaged alongside `wallset.exe` and `LICENSE`.

Review dependency license declarations and upstream license files whenever `Cargo.lock` changes, and update the notices when a dependency requires attribution or distribution of license text. Pay particular attention to unusual, copyleft, source-available, or custom terms.

## Asset provenance

| Asset | Provenance | Licensing status |
| --- | --- | --- |
| `assets/wallset.svg` | Original Wallset artwork added by project author `kibbi-bit` in the initial commit on 2026-08-16. | Covered by Wallset's MIT License. |
| `docs/MadeWithSlint-logo-whitebg.png` | Unmodified Slint attribution badge from the [Slint logo directory](https://github.com/slint-ui/slint/tree/v1.17.1/logo), added on 2026-08-16 for Slint attribution. | Slint-owned mark used for the attribution contemplated by the selected Slint license; it is not relicensed as Wallset artwork. |

Future images, fonts, icons, and copied code must be added to this table with their source and redistribution terms. Material with unknown or incompatible rights must be replaced before release.
