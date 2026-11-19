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
$toolchain = Join-Path $ndk 'toolchains/llvm/prebuilt/windows-x86_64'
$readelf = Join-Path $toolchain 'bin/llvm-readelf.exe'
$clang = Join-Path $toolchain 'bin/aarch64-linux-android26-clang.cmd'
$cxx = Join-Path $toolchain 'sysroot/usr/lib/aarch64-linux-android/libc++_shared.so'
$aapt2 = Join-Path $tools 'aapt2.exe'
$zipalign = Join-Path $tools 'zipalign.exe'
$apksigner = Join-Path $tools 'apksigner.bat'
$d8 = Join-Path $tools 'd8.bat'
foreach ($required in $aapt2, $zipalign, $apksigner, $d8, $readelf, $clang, $cxx, $platform) {
    if (!(Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Android SDK file is missing: $required"
    }
}
Get-Command keytool, javac -ErrorAction Stop | Out-Null

if ($LibraryPath -and !$OutputDirectory) {
    throw 'Pass -OutputDirectory with -LibraryPath to keep fixture APKs separate from the normal output.'
}
if (!$LibraryPath) {
    if (!(Test-Path -LiteralPath (Join-Path $root 'crates/meridian-android/Cargo.toml'))) {
        throw 'The meridian-android crate has not landed yet. Supply -LibraryPath to exercise packaging.'
    }
    $previousNdk = $env:ANDROID_NDK_HOME
    $previousClientId = $env:HORIZON_MSA_CLIENT_ID
    Push-Location -LiteralPath $root
    try {
        $env:ANDROID_NDK_HOME = $ndk
        if ($null -eq $previousClientId) {
            $env:HORIZON_MSA_CLIENT_ID = 'a3834636-8758-4bc7-9767-35c8ccc9d45d'
        } elseif ([string]::IsNullOrWhiteSpace($previousClientId)) {
            Write-Warning 'Microsoft sign-in disabled.'
        }
        cargo ndk -t arm64-v8a -P 26 build --locked -p meridian-android --profile $Profile --target-dir $cargoOutput
    } finally {
        $env:ANDROID_NDK_HOME = $previousNdk
        $env:HORIZON_MSA_CLIENT_ID = $previousClientId
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
$java = Join-Path $work 'libmeridian_java.so'
$classes = Join-Path $work 'classes'
try {
    & $clang (Join-Path $PSScriptRoot 'Java-Launcher.c') -ldl -o $java
    # MeridianActivity (java/) as classes.dex, with Starling's javac and d8 flags.
    $sources = (Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'java') -Recurse -Filter *.java).FullName
    javac --release 8 -nowarn '-Xlint:-options' -classpath $platform -d $classes $sources
    $compiled = (Get-ChildItem -LiteralPath $classes -Recurse -Filter *.class).FullName
    & $d8 --release --min-api 26 --lib $platform --output $work $compiled
    & $aapt2 link -o $unsigned --manifest (Join-Path $PSScriptRoot 'AndroidManifest.xml') -I $platform
    $zip = [IO.Compression.ZipFile]::Open($unsigned, [IO.Compression.ZipArchiveMode]::Update)
    try {
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $zip, $library, 'lib/arm64-v8a/libmeridian.so',
            [IO.Compression.CompressionLevel]::NoCompression
        ) | Out-Null
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $zip, (Join-Path $work 'classes.dex'), 'classes.dex'
        ) | Out-Null
        foreach ($native in $java, $cxx) {
            $name = Split-Path -Leaf $native
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                $zip, $native, "lib/arm64-v8a/$name",
                [IO.Compression.CompressionLevel]::NoCompression
            ) | Out-Null
        }
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
    foreach ($file in $unsigned, $aligned, $signed, $java) {
        if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file }
    }
    [IO.Directory]::Delete($work, $true)
}
