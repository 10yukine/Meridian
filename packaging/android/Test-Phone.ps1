#Requires -Version 7.4
[CmdletBinding()]
param(
    [string]$Serial,
    [string]$Apk,
    [string]$Resources,
    [switch]$InstallOnly
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
if (!$Apk) { $Apk = Join-Path $root 'target/android/meridian-dev-arm64-v8a.apk' }
$Apk = (Resolve-Path -LiteralPath $Apk).Path
if (!$env:ANDROID_HOME) { throw 'Set ANDROID_HOME to the Android SDK directory.' }
$aapt2 = Join-Path $env:ANDROID_HOME 'build-tools/36.0.0/aapt2.exe'
$app = ([xml](Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'AndroidManifest.xml'))).manifest.package
if ((& $aapt2 dump packagename $Apk).Trim() -cne $app) {
    throw "Expected an APK for $app."
}

if ($Resources) {
    $Resources = (Resolve-Path -LiteralPath $Resources).Path
    foreach ($marker in 'horizon-resources.json', 'horizon-prepare.json') {
        if (!(Test-Path -LiteralPath (Join-Path $Resources $marker) -PathType Leaf)) {
            throw "The resource folder is missing $marker. Prepare it using Meridian's horizon-prepare."
        }
    }
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $Resources 'horizon-resources.json') | ConvertFrom-Json
    $stamp = Get-Content -Raw -LiteralPath (Join-Path $Resources 'horizon-prepare.json') | ConvertFrom-Json
    $lock = Get-Content -Raw -LiteralPath (Join-Path $root 'research-lock.json') | ConvertFrom-Json
    $prepareSource = Get-Content -Raw -LiteralPath (Join-Path $root 'crates/horizon-prepare/src/lib.rs')
    $outputVersion = [regex]::Match($prepareSource, '(?m)^pub const OUTPUT_VERSION: u32 = (\d+);')
    if (!$outputVersion.Success -or $stamp.output_version -ne [int]$outputVersion.Groups[1].Value -or
        $manifest.minecraft -cne $lock.minecraft.version) {
        throw "Expected resources prepared for this Meridian checkout with horizon-prepare."
    }
}

$device = if ($Serial) { @('-s', $Serial) } else { @() }
if ((adb @device get-state).Trim() -ne 'device') { throw 'ADB device is not ready.' }
adb @device install -r $Apk

if ($Resources) {
    $destination = "files/.local/horizon-client/$($lock.minecraft.version)"
    # Fail before writing if a previous run already put resources here.
    adb @device shell run-as $app sh -c "'if test -e $destination; then echo Existing resources left intact. >&2; exit 1; fi'"
    $transferId = [Guid]::NewGuid().ToString('N')
    $staging = "$destination-$transferId"
    $archive = Join-Path ([IO.Path]::GetTempPath()) "meridian-$transferId.tar"
    $remoteArchive = "/data/local/tmp/meridian-$transferId.tar"
    try {
        tar.exe -cf $archive -C $Resources .
        adb @device push $archive $remoteArchive
        adb @device shell run-as $app mkdir -p $staging
        # The Android shell reads the upload, and run-as extracts it into private storage.
        adb @device shell "set -o pipefail; cat $remoteArchive | run-as $app tar -xf - -C $staging"
    } finally {
        if (Test-Path -LiteralPath $archive -PathType Leaf) {
            Remove-Item -LiteralPath $archive
        }
        try {
            adb @device shell rm -f $remoteArchive
        } catch {
            Write-Warning "Could not remove the temporary phone archive: $remoteArchive"
        }
    }
    adb @device shell run-as $app test -s "$staging/horizon-resources.json"
    adb @device shell run-as $app test -s "$staging/horizon-prepare.json"
    adb @device shell run-as $app mv -Tn $staging $destination
    adb @device shell run-as $app sh -c "'test ! -e $staging'"
    Write-Host "Prepared resources installed in app-private $destination."
}

if ($InstallOnly) { return }

$since = (adb @device shell "date '+%m-%d %H:%M:%S.000'").Trim()
$startInfo = $null
try {
    $startInfo = adb @device shell am start -W -n "$app/io.github.u10yukine.meridian.MeridianActivity"
    $startInfo | Write-Host
    if (!($startInfo -match '^Status: ok\s*$')) { throw 'Android did not report a successful launch.' }
    adb @device shell pidof $app | Out-Null
} finally {
    $logPath = Join-Path (Split-Path -Parent $Apk) ('phone-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff') + '.log')
    $startInfo | Set-Content -LiteralPath $logPath -Encoding utf8
    adb @device logcat -d -T $since -s Meridian RustStdoutStderr RustAndroidGlue AndroidRuntime ActivityTaskManager libc DEBUG |
        Add-Content -LiteralPath $logPath -Encoding utf8
    Write-Host "Startup log: $logPath"
}
