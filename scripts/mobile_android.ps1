param(
  [ValidateSet('init', 'build-debug', 'verify-debug')]
  [string]$Action = 'build-debug'
)

$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$toolchains = Join-Path $workspace '.toolchains'
$jdk = Get-ChildItem -LiteralPath (Join-Path $toolchains 'jdk17') -Directory | Select-Object -First 1
if (-not $jdk) { throw 'Local JDK 17 is missing under .toolchains/jdk17.' }

$env:JAVA_HOME = $jdk.FullName
$env:ANDROID_HOME = Join-Path $toolchains 'android-sdk'
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:NDK_HOME = Join-Path $env:ANDROID_HOME 'ndk/29.0.14206865'
$env:CARGO_TARGET_DIR = Join-Path $workspace '.target-mobile'
$apk = Join-Path $workspace 'src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk'

# The generated Android project is gitignored, but the Rust core hard-requires
# the Kotlin half of the mobile-recorder plugin (native_recorder.rs registers
# dev.fung.local.recorder.RecorderPlugin at startup — the app crashes on launch
# without it). Sync the tracked source into the generated tree on every init
# and build so regenerating gen/android can never lose it again.
function Sync-NativePlugin {
  $pluginSource = Join-Path $workspace 'src-tauri/mobile/android'
  if (-not (Test-Path -LiteralPath $pluginSource)) { throw "Tracked native plugin sources are missing at $pluginSource." }
  $pluginTargetDir = Join-Path $workspace 'src-tauri/gen/android/app/src/main/java'
  New-Item -ItemType Directory -Path $pluginTargetDir -Force | Out-Null
  Copy-Item -Path (Join-Path $pluginSource '*') -Destination $pluginTargetDir -Recurse -Force

  $manifest = Join-Path $workspace 'src-tauri/gen/android/app/src/main/AndroidManifest.xml'
  if (Test-Path -LiteralPath $manifest) {
    $content = Get-Content -LiteralPath $manifest -Raw
    if ($content -notmatch 'android\.permission\.RECORD_AUDIO') {
      $content = $content -replace '(<manifest[^>]*>)', "`$1`n    <uses-permission android:name=`"android.permission.RECORD_AUDIO`" />"
      Set-Content -LiteralPath $manifest -Value $content -Encoding utf8
    }
  }
}

switch ($Action) {
  'init' {
    Push-Location $workspace
    try {
      npx tauri android init --ci --skip-targets-install
      if ($LASTEXITCODE -ne 0) { throw "Tauri Android init failed with exit code $LASTEXITCODE." }
      Sync-NativePlugin
    }
    finally { Pop-Location }
  }
  'build-debug' {
    Push-Location $workspace
    try {
      Sync-NativePlugin
      npx tauri android build --debug --apk --target aarch64 --ci
      if ($LASTEXITCODE -ne 0) { throw "Tauri Android build failed with exit code $LASTEXITCODE." }
    }
    catch {
      $library = Join-Path $env:CARGO_TARGET_DIR 'aarch64-linux-android/debug/libfung_lib.so'
      if (-not (Test-Path -LiteralPath $library)) { throw }
      $newestRustInput = Get-ChildItem -LiteralPath (Join-Path $workspace 'src-tauri/src') -Recurse -File |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
      if ($newestRustInput -and (Get-Item -LiteralPath $library).LastWriteTimeUtc -lt $newestRustInput.LastWriteTimeUtc) {
        throw 'The Android Rust library is older than the source tree; refusing to package a stale native core.'
      }
      $jni = Join-Path $workspace 'src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a'
      New-Item -ItemType Directory -Path $jni -Force | Out-Null
      Copy-Item -LiteralPath $library -Destination (Join-Path $jni 'libfung_lib.so') -Force
      if (Test-Path -LiteralPath $apk) { Remove-Item -LiteralPath $apk -Force }
      Push-Location (Join-Path $workspace 'src-tauri/gen/android')
      try {
        .\gradlew.bat :app:assembleArm64Debug -x :app:rustBuildArm64Debug --no-daemon
        if ($LASTEXITCODE -ne 0) { throw "Gradle Android build failed with exit code $LASTEXITCODE." }
      }
      finally { Pop-Location }
    }
    finally { Pop-Location }
    if (-not (Test-Path -LiteralPath $apk)) { throw 'Android build completed without the expected APK.' }
    Get-Item -LiteralPath $apk
  }
  'verify-debug' {
    if (-not (Test-Path -LiteralPath $apk)) { throw 'Debug APK is missing. Run build-debug first.' }
    $apksigner = Join-Path $env:ANDROID_HOME 'build-tools/36.0.0/apksigner.bat'
    & $apksigner verify --verbose --print-certs $apk
    if ($LASTEXITCODE -ne 0) { throw "APK verification failed with exit code $LASTEXITCODE." }
    Get-FileHash -LiteralPath $apk -Algorithm SHA256
  }
}
