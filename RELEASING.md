# Releasing Wallset

Wallset releases are built from immutable semantic-version tags on `main`. Never rebuild, move, or replace an existing release tag; publish a new patch version instead.

## Publish a release

1. Select the next `MAJOR.MINOR.PATCH` version.
2. In a release preparation pull request, update `package.version` in `Cargo.toml` and refresh `Cargo.lock` with `cargo check`.
3. Merge the pull request after the required Windows CI checks pass.
4. Update local `main`, confirm the intended commit and version, then create and push an annotated tag:

   ```powershell
   git switch main
   git pull --ff-only
   git tag -a v0.1.0 -m "Wallset v0.1.0"
   git push origin v0.1.0
   ```

5. Verify that the **Release** GitHub Actions workflow succeeds.
6. Verify the GitHub Release points to the tagged commit, has generated release notes, and contains both `wallset-vX.Y.Z-windows-x86_64.zip` and its `.sha256` checksum.
7. Extract the ZIP and confirm it contains `wallset.exe`, `README.md`, `LICENSE`, and `THIRD_PARTY_NOTICES.md`. Independently verify the checksum:

   ```powershell
   Get-FileHash -Algorithm SHA256 .\wallset-v0.1.0-windows-x86_64.zip
   ```

The workflow accepts only stable tags in the exact form `vMAJOR.MINOR.PATCH`, and the version must match `Cargo.toml`. It rejects tagged commits that are not contained in `main`.

## Prereleases

A future prerelease could use a tag such as `v0.2.0-rc.1`, but that form is intentionally rejected today. Before creating one, extend the workflow's tag validation, map the tag to the Cargo prerelease version, and add `--prerelease` to release creation. Document and review that automation change before pushing the first prerelease tag.
