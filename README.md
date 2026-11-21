# Meridian

**An unofficial Android port of Horizon, the Rust reimplementation of Minecraft: Java Edition.**

Meridian brings Horizon to phones with a touch-first launcher and on-screen game controls. Android is the current focus. iOS is a possibility, with no release timeline.

> [!IMPORTANT]
> **Microsoft sign-in approval is pending.** We’ve submitted Meridian’s application to Microsoft for Minecraft sign-in access and are awaiting approval. Meridian is an independent project and has no affiliation with Microsoft or Mojang.

## Development status

Meridian is in active development. Android development builds can open the launcher and reach the game’s title screen on tested ARM64 phones. The launcher already supports profiles and on-device game setup. We’re continuing work on touch controls, the launcher UI, and gameplay on mobile.

## Build from source

The [Android build guide](packaging/android/README.md) covers the Rust and Android SDK prerequisites, how to build an ARM64 APK, and how to prepare game files from your own Minecraft: Java Edition installation. Meridian requires those game files and does not redistribute them.

## Upstream and licensing

Meridian is built on [Horizon](https://github.com/horizon-rust/horizon). It inherits Horizon’s LGPL-3.0-or-later license. See the [license](LICENSE), [additional licenses](LICENSES), and [third-party notices](THIRD_PARTY_NOTICES.md).
