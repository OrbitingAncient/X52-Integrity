# Security

## Report a vulnerability

Open a private GitHub security advisory on [OrbitingAncient/X52-Integrity](https://github.com/OrbitingAncient/X52-Integrity), or email the maintainer listed on the GitHub profile. Please include OS build, HVCI/Memory Integrity state, and `x52-integrity --self-test` output when it is relevant.

Do not file a public issue for an exploitable bug until we have a fix or an agreed disclosure date.

## What this project will not do

- Load unsigned kernel code
- Disable Memory Integrity, HVCI, or Smart App Control
- Flash device firmware
- Auto-elevate the GUI

See [docs/what-this-will-never-do.md](docs/what-this-will-never-do.md).

## Signing

Release binaries are **unsigned** until an Authenticode certificate is configured. Treat GitHub Actions artifacts as unsigned build outputs; verify SHA256 sums from the release notes. Signing steps: [docs/RELEASE.md](docs/RELEASE.md).

## Scope

The supported attack surface is the user-mode process talking to inbox HID / evdev. Logitech filter drivers (`SaiK075C`, `SaiU075C`) are third-party; we can bind away from them, we do not patch them.
