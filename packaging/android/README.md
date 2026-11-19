# Meridian Android APKs

Use PowerShell 7.4 or newer on Windows. Install the Rust `aarch64-linux-android` target, `cargo-ndk`, Android platform 36, Build Tools 36.0.0, NDK 28.2.13676358, `adb`, and JDK 25. Set `ANDROID_HOME` to the SDK and put the JDK's `bin` directory on PATH.

## Build

From the repository root:

```powershell
pwsh -File packaging/android/Build-Apk.ps1 -Profile fast
```

The APK is `target/android/meridian-dev-arm64-v8a.apk`. The package is `io.github.u10yukine.meridian.dev`, with Android 8.0 (API 26) as the minimum. The script aligns the APK for 16 KB pages and signs it with the local key at `$HOME/.android/meridian-debug.keystore`. Keep this key to install later debug builds over an existing copy and preserve its data.

## Build a signed release

Set these environment variables in the process that runs the build.

| Variable | Value |
| --- | --- |
| `MERIDIAN_RELEASE_KEYSTORE` | Path to the signing keystore |
| `MERIDIAN_RELEASE_KEY_ALIAS` | Signing key alias in that keystore |
| `MERIDIAN_RELEASE_STORE_PASS` | Keystore password |
| `MERIDIAN_RELEASE_KEY_PASS` | Signing key password |

```powershell
pwsh -File packaging/android/Build-Apk.ps1 -Release -VersionCode 1 -VersionName 0.1.0
```

`-Release` builds the non-debuggable `io.github.u10yukine.meridian` package using the Rust `release` profile by default and writes `target/android/meridian-release-arm64-v8a.apk`. The build reads the passwords from the environment and verifies the APK signature and 16 KB alignment. Keep the release keystore and passwords outside the repository. Use the same signing key and increase the version code for app updates.

When `CARGO_TARGET_DIR` is set, both builds use that target directory for native builds and default APK output.

## Prepare game files on the PC

Prepare from your own installation of the Minecraft version pinned by `research-lock.json` and the matching server JAR. Point `$minecraft` at your Java Edition installation and `$serverJar` at the pinned server JAR on this PC.

```powershell
$resources = Join-Path $PWD '.local/horizon-client/26.3'
cargo run --locked --profile fast -p horizon-prepare -- --prepare --minecraft-dir $minecraft --server-jar $serverJar --output $resources
```

Add `--complete` if your installation has the full assets and you want sounds and the full completeness check. Keep the prepared files local. They are separate from the APK.

## Run on a phone

```powershell
pwsh -File packaging/android/Test-Phone.ps1 -Resources $resources
```

Use `-Serial <adb-serial>` when multiple devices are connected. The script installs with `adb install -r`, copies the resource contents into the debug app's private `files/.local/horizon-client/26.3/` through ADB, then launches `MeridianActivity` (NativeActivity, `java/`) and saves startup logcat beside the APK. It uploads a temporary tar archive to Android's shell storage, extracts it under `run-as`, and removes the archive.

The capture includes game output under `RustStdoutStderr` and Android activity messages under `RustAndroidGlue`.

The resource copy is for the first setup. It refuses to replace existing resources and checks that the PC output matches the checkout's Minecraft and preparation versions. On repeat launches, omit `-Resources` to use the existing copy. Use `-InstallOnly` to stage the APK and optional resources without opening the app.

The native bootstrap smoke check ends when Android opens the activity and the app has a live process. Test rendering, touch, and save/resume separately on a disposable world.

## Launcher setup

The APK includes an executable Java launcher and the NDK C++ runtime. On first setup, Meridian downloads and verifies the pinned Android ARM64 Java 25 archive and prepares the Mojang resources in app-private storage.

Set `HORIZON_MSA_CLIENT_ID` before building to enable Microsoft device-code sign-in, using the same app registration as the desktop launcher. The offline setup flow needs no Microsoft account.
