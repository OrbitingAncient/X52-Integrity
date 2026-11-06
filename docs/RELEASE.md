# Release checklist

Version lives in `Cargo.toml` (`0.2.0`). Tag `v0.2.0`.

## Every release

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets -- -D warnings -W clippy::pedantic`
3. `cargo test`
4. Update `CHANGELOG` notes in the GitHub release body.
5. Push tag `vX.Y.Z`. GitHub Actions `release.yml` builds Windows zip + NSIS (if makensis is present) and Linux `.deb` + AppDir.

## Windows signing (Authenticode)

Smart App Control and SmartScreen need a signature. After CI (or locally):

```powershell
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `
  /a dist\X52-Integrity-0.2.0-setup.exe
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `
  /a dist\X52-Integrity-0.2.0-windows-x64-portable.zip
```

Use an OV or EV code-signing cert. EV establishes SmartScreen reputation faster. CI does **not** sign; it uploads unsigned artifacts so you can sign offline.

MSIX (optional): fill `packaging/windows/AppxManifest.xml` publisher CN to match the cert, then `MakeAppx pack` and `signtool sign`.

## Linux

- `.deb` via `cargo deb` (see `Cargo.toml` `[package.metadata.deb]`).
- AppImage via `packaging/linux/build-appimage.sh` when `appimagetool` is installed.

No kernel module, so no module signing.

## Manual local pack (Windows)

```powershell
cargo build --release
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging\windows\build-portable.ps1
makensis /DVERSION=0.2.0 /DRELEASEDIR=target\release packaging\windows\x52-integrity.nsi
```

## GitHub release contents

- `X52-Integrity-VERSION-windows-x64-portable.zip`
- `X52-Integrity-VERSION-setup.exe` (NSIS, if built)
- `x52-integrity_VERSION_amd64.deb`
- `X52-Integrity-VERSION-x86_64.AppImage` (if appimagetool present)
- SHA256 sums

## After publishing

- Confirm the portable zip runs on a standard-user account.
- Confirm `--install --silent` writes `install-manifest.json` and `logs\install.log`.
- Confirm uninstall removes shortcuts.
- Do not attach Logitech `.sys` / `.cat` files to the release.
