<p align="center">
  <img src="https://img.shields.io/badge/Minecraft-Java%2026.3-5D9C3F" alt="Minecraft Java 26.3">
  <img src="https://img.shields.io/badge/platform-Android-6D5ED1" alt="Android">
  <a href="https://github.com/horizon-rust/horizon"><img src="https://img.shields.io/badge/fork%20of-Horizon-F0795A" alt="Fork of Horizon"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-LGPL--3.0--or--later-3B2F7E" alt="LGPL-3.0-or-later license"></a>
</p>

<p align="center">
  <a href="#download"><b>Download</b></a> ·
  <a href="#what-works-today"><b>What works</b></a> ·
  <a href="packaging/android/README.md"><b>Build guide</b></a> ·
  <a href="CONTRIBUTING.md"><b>Contributing</b></a> ·
  <a href="https://github.com/horizon-rust/horizon"><b>Horizon</b></a>
</p>

# Meridian

**An unofficial Android port of Horizon, the Rust reimplementation of Minecraft: Java Edition.**

Meridian brings Horizon to phones with a touch-first launcher and on-screen game controls. Android is the current focus. iOS is a possibility, with no release timeline.

> [!IMPORTANT]
> **Microsoft sign-in approval is pending.** We’ve submitted Meridian’s application to Microsoft for Minecraft sign-in access and are awaiting approval. Meridian is an independent project and has no affiliation with Microsoft or Mojang.

## Download

Test builds are on the [releases page](https://github.com/10yukine/Meridian/releases). They are early: expect rough edges, and say what you find.

**You need** an Android 8.0 or newer phone with a 64-bit (ARM64) processor and about 1.5 GB free. On the first start, Meridian downloads the game’s files from Mojang and prepares them on the phone, in about two minutes on a mid-range phone.

## What works today

- **Setting up on the phone:** the launcher downloads the game from Mojang and prepares it on the phone. Music and the other languages follow in the background, so you can play sooner.
- **The launcher:** profiles, mods from Horizon’s registry in their own profiles, skins with a 3D preview, and the game’s log.
- **The game:** Horizon’s singleplayer, with on-screen touch controls.
- **Accounts:** offline names and Ely.by now; Microsoft once sign-in is approved.

Tested on a Galaxy A14 5G (Mali-G57, Android 15) and a Galaxy A32 (Mali-G52, Android 13). Phones with Snapdragon (Adreno) graphics haven’t been tried yet.

## Build from source

The [Android build guide](packaging/android/README.md) covers the Rust and Android SDK prerequisites, how to build an ARM64 APK, and how to prepare game files from your own Minecraft: Java Edition installation. Meridian requires those game files and does not redistribute them.

## Contributing

Changes to the game itself belong upstream, in [Horizon](https://github.com/horizon-rust/horizon); Meridian is the Android app around it. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

Meridian is built on [Horizon](https://github.com/horizon-rust/horizon) and inherits its [LGPL-3.0-or-later](LICENSE) licence for the project’s original work. Code Horizon merged before its move from MIT stays available under the [MIT licence](LICENSES/MIT.txt), as does code ported from other MIT projects. Dependencies keep their own licences: see the [third-party notices](THIRD_PARTY_NOTICES.md).

## Authors

Meridian by [@10yukine](https://github.com/10yukine). Horizon by [Ludovic Marie](https://github.com/ludovic111), the Horizon Team and [its contributors](https://github.com/horizon-rust/horizon/graphs/contributors).

## Minecraft and the EULA

Meridian downloads the game’s files from Mojang’s servers on your phone and never hosts or redistributes them.

<sub>NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT. Minecraft is a trademark of Mojang Synergies AB; its code and assets belong to Mojang and Microsoft under the [Minecraft EULA](https://www.minecraft.net/en-us/eula).</sub>
