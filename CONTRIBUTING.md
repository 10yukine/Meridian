# Contributing to Meridian

Meridian is an unofficial Android port of [Horizon](https://github.com/horizon-rust/horizon), the Rust reimplementation of Minecraft: Java Edition. Most of this repository is Horizon itself, kept close to upstream and synced with it every few days.

## Where a change belongs

- **The game:** the engine, rendering, worlds, gameplay and mods come from Horizon. Send those changes to Horizon ([its contributing guide](https://github.com/horizon-rust/horizon/blob/main/CONTRIBUTING.md)). They reach Meridian with the next sync.
- **Android:** open a pull request here for the launcher (`crates/horizon-client/src/mobile_launcher/`, `crates/meridian-launcher/`), the Android entry point (`crates/meridian-android/`), touch controls, and packaging (`packaging/android/`).

## Build and test

The [Android build guide](packaging/android/README.md) lists the prerequisites and builds an APK. Run the launcher's tests before opening a pull request:

```sh
cargo test -p meridian-launcher
cargo test -p horizon-client-lib --lib mobile_launcher
```

Test Android changes on a phone, and say which phone in the pull request.

## Keep the diff small

Change Horizon's files only where Android needs it, and keep those changes small, so each sync with upstream stays easy. Gate Android-only code with `cfg(target_os = "android")`.

## Licensing

Contributions are licensed under the repository's license, LGPL-3.0-or-later ([LICENSE](LICENSE)). Code ported from another project keeps its copyright line and gets an entry in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
