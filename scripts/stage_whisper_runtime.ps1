[CmdletBinding()]
param(
    [string]$PythonVersion = '3.11.9',
    [string]$FasterWhisperVersion = '1.2.1',
    [ValidateSet('small', 'large-v3-turbo', 'medium', 'large-v3')]
    [string]$Model = 'large-v3-turbo',
    [string]$ModelRevision,
    [string]$HostPython,
    [switch]$Clean
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $repoRoot '.venv-whisper'
$cacheRoot = Join-Path $repoRoot '.runtime-cache'
$pythonUrl = "https://www.python.org/ftp/python/$PythonVersion/python-$PythonVersion-embed-amd64.zip"
$pythonSha256 = '009d6bf7e3b2ddca3d784fa09f90fe54336d5b60e0f0e305c37f400bf83cfd3b'
$requirements = Join-Path $PSScriptRoot 'whisper-runtime-requirements.txt'
$modelRepos = @{
    'small' = 'Systran/faster-whisper-small'
    'large-v3-turbo' = 'mobiuslabsgmbh/faster-whisper-large-v3-turbo'
    'medium' = 'Systran/faster-whisper-medium'
    'large-v3' = 'Systran/faster-whisper-large-v3'
}
$modelRepo = $modelRepos[$Model]

if ($Clean) {
    if (Test-Path -LiteralPath $destination) {
        $resolvedRepo = (Resolve-Path -LiteralPath $repoRoot).Path.TrimEnd('\')
        $resolvedDestination = (Resolve-Path -LiteralPath $destination).Path
        if (-not $resolvedDestination.StartsWith("$resolvedRepo\", [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove runtime outside repository: $resolvedDestination"
        }
        Remove-Item -LiteralPath $resolvedDestination -Recurse -Force
    }
    Write-Host 'Removed staged FUNG Whisper runtime.'
    exit 0
}

if ($PythonVersion -ne '3.11.9' -or $FasterWhisperVersion -ne '1.2.1') {
    throw 'This release script accepts only the reviewed Python 3.11.9 / faster-whisper 1.2.1 runtime.'
}

$scriptsDir = Join-Path $destination 'Scripts'
$sitePackages = Join-Path $destination 'Lib\site-packages'
$modelDir = Join-Path $destination (Join-Path 'models' $Model)
$licensesDir = Join-Path $destination 'LICENSES'
New-Item -ItemType Directory -Path $scriptsDir, $sitePackages, $modelDir, $licensesDir -Force | Out-Null

$embeddedPython = Join-Path $scriptsDir 'python.exe'
if (-not (Test-Path -LiteralPath $embeddedPython -PathType Leaf)) {
    if (-not $HostPython) {
        $uv = Get-Command uv -ErrorAction SilentlyContinue
        if ($uv) {
            $HostPython = (& $uv.Source python find 3.11).Trim()
        }
    }
    if (-not $HostPython -or -not (Test-Path -LiteralPath $HostPython -PathType Leaf)) {
        throw 'A Python 3.11 build interpreter is required. Pass -HostPython or install it with uv.'
    }

    New-Item -ItemType Directory -Path $cacheRoot -Force | Out-Null
    $pythonArchive = Join-Path $cacheRoot "python-$PythonVersion-embed-amd64.zip"
    if (-not (Test-Path -LiteralPath $pythonArchive -PathType Leaf)) {
        Invoke-WebRequest -Uri $pythonUrl -OutFile $pythonArchive
    }
    $actualPythonHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $pythonArchive).Hash.ToLowerInvariant()
    if ($actualPythonHash -ne $pythonSha256) {
        throw "Python archive SHA-256 mismatch: expected $pythonSha256, got $actualPythonHash"
    }

    Expand-Archive -LiteralPath $pythonArchive -DestinationPath $scriptsDir -Force

    $pthPath = Join-Path $scriptsDir 'python311._pth'
    @(
        'python311.zip'
        '.'
        '..\Lib\site-packages'
        'import site'
    ) | Set-Content -LiteralPath $pthPath -Encoding ascii

    & $HostPython -m pip install --disable-pip-version-check --no-deps --only-binary=:all: --require-hashes --target $sitePackages -r $requirements
    if ($LASTEXITCODE -ne 0) {
        throw "Pinned runtime dependency install failed with exit code $LASTEXITCODE"
    }
} else {
    Write-Host "Reusing staged Python runtime at $scriptsDir"
}

$manifestPath = Join-Path $destination 'manifest.json'
$existingManifest = $null
if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
    try {
        $existingManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    } catch {
        throw "Existing Whisper runtime manifest is invalid: $manifestPath"
    }
}

$resolvedModelRevision = $ModelRevision
if (-not $resolvedModelRevision -and $existingManifest -and $existingManifest.models) {
    $existingModel = @($existingManifest.models) | Where-Object { $_.name -eq $Model } | Select-Object -First 1
    if ($existingModel) {
        $resolvedModelRevision = $existingModel.revision
    }
}
if (-not $resolvedModelRevision -and $Model -eq 'small') {
    $resolvedModelRevision = '536b0662742c02347bc0e980a01041f333bce120'
}
if (-not $resolvedModelRevision) {
    $resolveRevisionCode = @"
from huggingface_hub import HfApi
print(HfApi().model_info('$modelRepo').sha)
"@
    $resolvedModelRevision = (& $embeddedPython -c $resolveRevisionCode | Select-Object -Last 1).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $resolvedModelRevision -or $resolvedModelRevision -notmatch '^[0-9a-f]{40}$') {
        throw "Could not resolve a pinned Hugging Face revision for $modelRepo. Pass -ModelRevision explicitly."
    }
}

$modelFiles = @('config.json', 'preprocessor_config.json', 'model.bin', 'tokenizer.json', 'vocabulary.*')
$modelReady = Test-Path -LiteralPath (Join-Path $modelDir 'model.bin') -PathType Leaf
if (-not $modelReady) {
    $allowPatterns = $modelFiles | ConvertTo-Json -Compress
    $downloadCode = @"
from huggingface_hub import snapshot_download
snapshot_download(
    repo_id='$modelRepo',
    revision='$resolvedModelRevision',
    local_dir=r'$modelDir',
    allow_patterns=$allowPatterns,
)
"@
    # huggingface_hub reports download progress on stderr; under Windows
    # PowerShell 5.1 with Stop, that progress can look like a native error.
    # Merge the streams and judge the download by its exit code only.
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $embeddedPython -c $downloadCode 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -notmatch '%\|' } | Out-Host
    $downloadExit = $LASTEXITCODE
    $ErrorActionPreference = $previousPreference
    if ($downloadExit -ne 0) {
        throw "Pinned Whisper model download failed with exit code $downloadExit"
    }
} else {
    Write-Host "Reusing staged Whisper model at $modelDir"
}

$modelSlug = $Model.Replace('-', '_')
$licenseSources = [ordered]@{
    'PYTHON-LICENSE.txt' = "https://raw.githubusercontent.com/python/cpython/v$PythonVersion/LICENSE"
    'FASTER-WHISPER-LICENSE.txt' = "https://raw.githubusercontent.com/SYSTRAN/faster-whisper/v$FasterWhisperVersion/LICENSE"
    "WHISPER-$modelSlug-MODEL-CARD.md" = "https://huggingface.co/$modelRepo/raw/$resolvedModelRevision/README.md"
}
foreach ($entry in $licenseSources.GetEnumerator()) {
    $licensePath = Join-Path $licensesDir $entry.Key
    if (-not (Test-Path -LiteralPath $licensePath -PathType Leaf)) {
        Invoke-WebRequest -Uri $entry.Value -OutFile $licensePath
    }
}

$probeCode = @"
from faster_whisper import WhisperModel
model = WhisperModel(r'$modelDir', device='cpu', compute_type='int8')
print('portable-whisper-ready')
"@
$probeOutput = & $embeddedPython -c $probeCode
if ($LASTEXITCODE -ne 0 -or $probeOutput -notcontains 'portable-whisper-ready') {
    throw 'Portable Whisper runtime probe failed.'
}

$files = Get-ChildItem -LiteralPath $destination -Recurse -File | Where-Object {
    $_.FullName -ne $manifestPath
} | ForEach-Object {
    if (-not $_.FullName.StartsWith("$destination\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Runtime manifest encountered a file outside destination: $($_.FullName)"
    }
    [ordered]@{
        path = $_.FullName.Substring($destination.Length).TrimStart('\').Replace('\', '/')
        bytes = $_.Length
        sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
    }
}
$previousModels = @()
if ($existingManifest -and $existingManifest.models) {
    $previousModels = @($existingManifest.models) | Where-Object { $_.name -ne $Model }
} elseif ($existingManifest -and $existingManifest.model -and $existingManifest.model.name -ne $Model) {
    $previousModels = @($existingManifest.model)
}
$currentModel = [ordered]@{
    name = $Model
    repository = $modelRepo
    revision = $resolvedModelRevision
    license = 'MIT'
}
$manifest = [ordered]@{
    schemaVersion = 2
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    profile = 'multi-model'
    defaultModel = 'large-v3-turbo'
    python = [ordered]@{ version = $PythonVersion; source = $pythonUrl; sha256 = $pythonSha256 }
    fasterWhisper = [ordered]@{ version = $FasterWhisperVersion; requirements = 'scripts/whisper-runtime-requirements.txt' }
    models = @($previousModels) + @($currentModel)
    files = @($files)
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding utf8

Write-Host "Staged portable FUNG Whisper runtime at $destination"
Write-Host "Model: $Model ($resolvedModelRevision)"
Write-Host "Files: $($files.Count); bytes: $((Get-ChildItem -LiteralPath $destination -Recurse -File | Measure-Object Length -Sum).Sum)"
