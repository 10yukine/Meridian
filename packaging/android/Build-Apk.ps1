#Requires -Version 7.4
[CmdletBinding()]
param(
    [ValidateSet('release', 'fast')]
    [string]$Profile = 'release',
    [string]$LibraryPath,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$cargoOutput = Join-Path $root 'target'

if (!$env:ANDROID_HOME) { throw 'Set ANDROID_HOME to the Android SDK directory.' }
$tools = Join-Path $env:ANDROID_HOME 'build-tools/36.0.0'
$platform = Join-Path $env:ANDROID_HOME 'platforms/android-36/android.jar'
$ndk = if ($env:ANDROID_NDK_HOME) { $env:ANDROID_NDK_HOME } else {
    Join-Path $env:ANDROID_HOME 'ndk/28.2.13676358'
}
$readelf = Join-Path $ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-readelf.exe'
$aapt2 = Join-Path $tools 'aapt2.exe'
$zipalign = Join-Path $tools 'zipalign.exe'
$apksigner = Join-Path $tools 'apksigner.bat'
foreach ($required in $aapt2, $zipalign, $apksigner, $readelf, $platform) {
    if (!(Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Android SDK file is missing: $required"
    }
}
Get-Command keytool -ErrorAction Stop | Out-Null

if ($LibraryPath -and !$OutputDirectory) {
    throw 'Pass -OutputDirectory with -LibraryPath to keep fixture APKs separate from the normal output.'
}
if (!$LibraryPath) {
    if (!(Test-Path -LiteralPath (Join-Path $root 'crates/meridian-android/Cargo.toml'))) {
        throw 'The meridian-android crate has not landed yet. Supply -LibraryPath to exercise packaging.'
    }
    $previousNdk = $env:ANDROID_NDK_HOME
    Push-Location -LiteralPath $root
    try {
        $env:ANDROID_NDK_HOME = $ndk
        cargo ndk -t arm64-v8a -P 26 build --locked -p meridian-android --profile $Profile --target-dir $cargoOutput
    } finally {
        $env:ANDROID_NDK_HOME = $previousNdk
        Pop-Location
    }
    $LibraryPath = Join-Path $cargoOutput "aarch64-linux-android/$Profile/libmeridian.so"
}

$library = (Resolve-Path -LiteralPath $LibraryPath).Path
$elf = & $readelf -h -lW $library
$segments = @($elf -match '^\s*LOAD\s')
if (!($elf -match '^\s*Machine:\s*AArch64\s*$') -or
    !($elf -match '^\s*Type:\s*DYN\b') -or $segments.Length -eq 0) {
    throw "Expected an AArch64 shared library: $library"
}
foreach ($segment in $segments) {
    if ($segment -notmatch '\s0x([0-9a-f]+)\s*$' -or
        [Convert]::ToInt64($Matches[1], 16) -lt 16384) {
        throw "The library's LOAD segments must be aligned for 16 KB pages: $segment"
    }
}
if (!((& $readelf --dyn-syms --wide $library) -match '\s+GLOBAL\s+DEFAULT\s+\d+\s+ANativeActivity_onCreate\s*$')) {
    throw 'The library does not export ANativeActivity_onCreate.'
}

if (!$OutputDirectory) { $OutputDirectory = Join-Path $cargoOutput 'android' }
[IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null
$out = (Resolve-Path -LiteralPath $OutputDirectory).Path
$apk = Join-Path $out 'meridian-dev-arm64-v8a.apk'
$debugKey = Join-Path $HOME '.android/meridian-debug.keystore'
if (!(Test-Path -LiteralPath $debugKey -PathType Leaf)) {
    [IO.Directory]::CreateDirectory((Split-Path -Parent $debugKey)) | Out-Null
    keytool -genkeypair -storetype PKCS12 -keystore $debugKey -storepass android -keypass android `
        -alias meridian-debug -keyalg RSA -keysize 2048 -validity 10000 -dname 'CN=Meridian Debug'
}

$work = [IO.Directory]::CreateTempSubdirectory('meridian-apk-').FullName
$unsigned = Join-Path $work 'unsigned.apk'
$aligned = Join-Path $work 'aligned.apk'
$signed = Join-Path $work 'signed.apk'
try {
    & $aapt2 link -o $unsigned --manifest (Join-Path $PSScriptRoot 'AndroidManifest.xml') -I $platform
    $zip = [IO.Compression.ZipFile]::Open($unsigned, [IO.Compression.ZipArchiveMode]::Update)
    try {
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $zip, $library, 'lib/arm64-v8a/libmeridian.so',
            [IO.Compression.CompressionLevel]::NoCompression
        ) | Out-Null
    } finally {
        $zip.Dispose()
    }
    & $zipalign -P 16 -f 4 $unsigned $aligned
    & $apksigner sign --ks $debugKey --ks-key-alias meridian-debug --ks-pass pass:android `
        --key-pass pass:android --v4-signing-enabled false --out $signed $aligned
    & $apksigner verify $signed
    & $zipalign -c -P 16 4 $signed
    Move-Item -LiteralPath $signed -Destination $apk -Force
    Write-Output $apk
} finally {
    foreach ($file in $unsigned, $aligned, $signed) {
        if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file }
    }
    [IO.Directory]::Delete($work)
}
